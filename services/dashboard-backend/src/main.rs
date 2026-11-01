use axum::{routing::get, Router};
use cbtc_auth::JwtCodec;
use cbtc_config::AppConfig;
use cbtc_mq::MqClient;
use cbtc_ptdf::PtdfRecorder;
use dashboard_backend::{
    global_feed_engine::run_global_feed_engine,
    mq_consumer,
    router,
    state::AppState,
};
use sqlx::Row;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load("dashboard-backend")?;
    cbtc_observability::init_tracing(&cfg.service_name);
    let metrics_handle = cbtc_observability::init_metrics();
    let pool = cbtc_db::init_pool(&cfg.database).await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;

    let ptdf = Arc::new(PtdfRecorder::new(pool.clone()));
    let existing_run_id = sqlx::query(
        "SELECT id FROM ptdf.runs WHERE status = 'recording' ORDER BY started_at DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await?
    .map(|row| row.get("id"));

    // PTDF capture is a backend responsibility. If the service restarts,
    // resume an active trace; otherwise create a new continuous trace.
    let active_run_id = if let Some(id) = existing_run_id {
        tracing::info!(run_id = %id, "resuming continuous PTDF recording");
        Some(id)
    } else {
        let id = ptdf.start_run("Continuous platform recording").await?;
        tracing::info!(run_id = %id, "started continuous PTDF recording");
        Some(id)
    };

    let mq = Arc::new(MqClient::connect(&cfg.rabbitmq, cfg.fault_injection.clone()).await?.with_recording(ptdf.clone(), active_run_id.unwrap()));
    let jwt_codec = Arc::new(JwtCodec::new(&cfg.auth.jwt_secret, cfg.auth.jwt_ttl_secs));
    let state = AppState::new(
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build()?,
        std::env::var("CENTRAL_CONTROL_SERVICE_URL")
            .unwrap_or_else(|_| "http://central-control-service:8080".to_string()),
        pool.clone(), ptdf, active_run_id,
    );

    let global_pool = pool.clone();
    let global_ptdf = state.ptdf.clone();
    let global_run_id = active_run_id;
    tokio::spawn(async move {
        if let Err(error) = run_global_feed_engine(global_pool, global_ptdf, global_run_id).await {
            tracing::error!(error = %error, "global railway feed engine terminated");
        }
    });

    let consumer_mq = mq.clone();
    let consumer_state = state.clone();
    tokio::spawn(async move {
        if let Err(error) = mq_consumer::run(consumer_mq, consumer_state).await {
            tracing::error!(error = %error, "dashboard-backend MQ consumer terminated unexpectedly");
        }
    });

    let ready_pool = pool.clone();
    let app = Router::new()
        .merge(router::build_router(state, jwt_codec))
        .route("/ready", get(move || {
            let pool = ready_pool.clone();
            async move {
                if cbtc_db::health_check(&pool).await {
                    (axum::http::StatusCode::OK, "ready")
                } else {
                    (axum::http::StatusCode::SERVICE_UNAVAILABLE, "database unavailable")
                }
            }
        }))
        .merge(cbtc_observability::observability_routes(metrics_handle));

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    tracing::info!(addr = %addr, "dashboard-backend listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}




