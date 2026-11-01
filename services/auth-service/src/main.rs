use auth_service::{router, state::AppState};
use axum::{routing::get, Router};
use cbtc_auth::JwtCodec;
use cbtc_config::AppConfig;
use cbtc_mq::MqClient;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load("auth-service")?;
    cbtc_observability::init_tracing(&cfg.service_name);
    let metrics_handle = cbtc_observability::init_metrics();

    let pool = cbtc_db::init_pool(&cfg.database).await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;

    let mq = MqClient::connect(&cfg.rabbitmq, cfg.fault_injection.clone()).await?;
    let jwt_codec = Arc::new(JwtCodec::new(&cfg.auth.jwt_secret, cfg.auth.jwt_ttl_secs));

    let state = AppState {
        pool: pool.clone(),
        jwt_codec: jwt_codec.clone(),
        mq: Arc::new(mq),
    };

    let ready_pool = pool.clone();
    let app = Router::new()
        .merge(router::build_router(state, jwt_codec))
        .route(
            "/ready",
            get(move || {
                let pool = ready_pool.clone();
                async move {
                    if cbtc_db::health_check(&pool).await {
                        (axum::http::StatusCode::OK, "ready")
                    } else {
                        (
                            axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            "database unavailable",
                        )
                    }
                }
            }),
        )
        .merge(cbtc_observability::observability_routes(metrics_handle));

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    tracing::info!(addr = %addr, "auth-service listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
