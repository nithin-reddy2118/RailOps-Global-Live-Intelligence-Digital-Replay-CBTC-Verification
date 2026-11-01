use api_gateway::router;
use axum::{routing::get, Router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cbtc_observability::init_tracing("api-gateway");
    let metrics_handle = cbtc_observability::init_metrics();

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let requests_per_second: u64 = std::env::var("GATEWAY_RATE_LIMIT_RPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);

    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let app = Router::new()
        .route("/ready", get(|| async { "ready" }))
        .merge(cbtc_observability::observability_routes(metrics_handle))
        .merge(router::build_router(http, requests_per_second));

    let addr = format!("{host}:{port}");
    tracing::info!(addr = %addr, requests_per_second, "api-gateway listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
