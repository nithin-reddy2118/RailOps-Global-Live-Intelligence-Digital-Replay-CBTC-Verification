use axum::{routing::get, Router};
use cbtc_auth::JwtCodec;
use cbtc_config::AppConfig;
use cbtc_mq::MqClient;
use comms_gateway::{
    router,
    state::{AppState, BlackoutRegistry},
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load("comms-gateway")?;
    cbtc_observability::init_tracing(&cfg.service_name);
    let metrics_handle = cbtc_observability::init_metrics();

    let mq = MqClient::connect(&cfg.rabbitmq, cfg.fault_injection.clone()).await?;
    let jwt_codec = Arc::new(JwtCodec::new(&cfg.auth.jwt_secret, cfg.auth.jwt_ttl_secs));

    let state = AppState {
        jwt_codec: jwt_codec.clone(),
        mq: Arc::new(mq),
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?,
        train_service_base_url: std::env::var("TRAIN_SERVICE_URL")
            .unwrap_or_else(|_| "http://train-service:8080".to_string()),
        fault_injection: cfg.fault_injection.clone(),
        blackouts: Arc::new(BlackoutRegistry::default()),
    };

    let app = Router::new()
        .merge(router::build_router(state, jwt_codec))
        .route("/ready", get(|| async { "ready" }))
        .merge(cbtc_observability::observability_routes(metrics_handle));

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    tracing::info!(addr = %addr, "comms-gateway listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
