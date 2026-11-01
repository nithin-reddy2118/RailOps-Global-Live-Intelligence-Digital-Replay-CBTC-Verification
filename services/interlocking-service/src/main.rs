use axum::{routing::get, Router};
use cbtc_auth::JwtCodec;
use cbtc_config::AppConfig;
use cbtc_mq::MqClient;
use interlocking_service::{
    mq_consumer, router,
    state::{AppState, PeerServices},
    topology_cache::TopologyCache,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load("interlocking-service")?;
    cbtc_observability::init_tracing(&cfg.service_name);
    let metrics_handle = cbtc_observability::init_metrics();

    let pool = cbtc_db::init_pool(&cfg.database).await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;

    let mq = MqClient::connect(&cfg.rabbitmq, cfg.fault_injection.clone()).await?;
    let jwt_codec = Arc::new(JwtCodec::new(&cfg.auth.jwt_secret, cfg.auth.jwt_ttl_secs));

    let topology = Arc::new(TopologyCache::load(&pool).await?);

    let peers = PeerServices {
        movement_authority_base_url: std::env::var("MOVEMENT_AUTHORITY_SERVICE_URL")
            .unwrap_or_else(|_| "http://movement-authority-service:8080".to_string()),
        service_token: std::env::var("INTERLOCKING_SERVICE_TOKEN").expect(
            "INTERLOCKING_SERVICE_TOKEN must be set (a pre-issued operator-role service JWT)",
        ),
    };

    let state = AppState {
        pool: pool.clone(),
        jwt_codec: jwt_codec.clone(),
        mq: Arc::new(mq),
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?,
        peers,
        topology,
    };

    let consumer_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = mq_consumer::run(consumer_state).await {
            tracing::error!(error = %e, "interlocking-service MQ consumer terminated unexpectedly");
        }
    });

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
    tracing::info!(addr = %addr, "interlocking-service listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
