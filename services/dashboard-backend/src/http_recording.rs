use axum::{extract::State, extract::Request, middleware::Next, response::Response};
use serde_json::json;
use std::time::Instant;

use crate::state::AppState;

pub async fn record_http(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let started = Instant::now();

    let response = next.run(request).await;

    let status = response.status().as_u16();
    let duration_ms = started.elapsed().as_millis() as u64;

    if let Some(run_id) = *state.active_run_id.read().await {
        let payload = json!({
            "method": method,
            "path": path,
            "status": status,
            "duration_ms": duration_ms
        });

        if let Err(error) = state
            .ptdf
            .record_platform_event(
                run_id,
                "platform.http.request",
                "dashboard-backend",
                payload,
            )
            .await
        {
            tracing::warn!(%error, "failed to record HTTP request in PTDF");
        }
    }

    response
}
