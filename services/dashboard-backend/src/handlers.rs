use crate::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
    Json,
};
use cbtc_auth::{AuthenticatedUser, Role};
use cbtc_errors::AppError;
use cbtc_ptdf::PtdfReplayer;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;


#[derive(Debug, serde::Deserialize)]
pub struct PlatformEventRequest {
    pub event_type: String,
    pub origin_service: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

pub async fn platform_event(
    State(state): State<AppState>,
    Json(request): Json<PlatformEventRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let run_id = state
        .active_run_id
        .read()
        .await
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("continuous PTDF recording is not active")))?;

    state
        .ptdf
        .record_platform_event(
            run_id,
            &request.event_type,
            &request.origin_service,
            request.payload,
        )
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?;

    Ok(Json(serde_json::json!({
        "recorded": true
    })))
}
pub async fn websocket_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.event_tx.subscribe();

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(json) => {
                        if socket.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::debug!(
                            skipped,
                            "dashboard websocket client lagged; some events dropped"
                        );
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = socket.recv() => {
                if incoming.is_none() {
                    break;
                }
            }
        }
    }
}

pub async fn snapshot(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let url = format!("{}/control/overview", state.central_control_base_url);

    let response = state
        .http
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::UpstreamService {
            service: "central-control-service",
            status: e.status().map(|s| s.as_u16()).unwrap_or(0),
        })?;

    if !response.status().is_success() {
        return Err(AppError::UpstreamService {
            service: "central-control-service",
            status: response.status().as_u16(),
        });
    }

    let body: Value = response
        .json()
        .await
        .map_err(|_| AppError::UpstreamService {
            service: "central-control-service",
            status: 200,
        })?;

    Ok(Json(body))
}

#[derive(Debug, Serialize, FromRow)]
pub struct StationOverview {
    pub id: Uuid,
    pub name: String,
    pub country_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub station_code: Option<String>,
    pub operator: Option<String>,
    pub source: String,
}

pub async fn stations(
    State(state): State<AppState>,
) -> Result<Json<Vec<StationOverview>>, AppError> {
    let rows = sqlx::query_as::<_, StationOverview>(
        r#"
        SELECT
            id,
            name,
            country_code::text AS country_code,
            latitude,
            longitude,
            station_code,
            operator,
            source
        FROM tracking.stations
        ORDER BY name ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

#[derive(Debug, Serialize, FromRow)]
pub struct ReplayRunSummary {
    pub id: Uuid,
    pub label: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: String,
    pub event_count: i64,
    pub active: bool,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ReplayRunDetail {
    pub id: Uuid,
    pub label: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: String,
    pub event_count: i64,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct StartRunRequest {
    pub label: String,
}

async fn require_admin(user: &AuthenticatedUser) -> Result<(), AppError> {
    user.require_role(Role::Admin)
}

pub async fn list_runs(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<ReplayRunSummary>>, AppError> {
    require_admin(&user).await?;

    let active_run_id = *state.active_run_id.read().await;

    let rows = sqlx::query_as::<_, ReplayRunSummary>(
        r#"
        SELECT
            r.id,
            r.label,
            r.started_at,
            r.ended_at,
            r.status,
            COUNT(t.id)::BIGINT AS event_count,
            ($1::UUID IS NOT NULL AND r.id = $1::UUID) AS active
        FROM ptdf.runs r
        LEFT JOIN ptdf.trace_entries t ON t.run_id = r.id
        GROUP BY r.id
        ORDER BY r.started_at DESC
        "#,
    )
    .bind(active_run_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

pub async fn start_run(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(request): Json<StartRunRequest>,
) -> Result<Json<ReplayRunSummary>, AppError> {
    require_admin(&user).await?;

    let label = request.label.trim();

    if label.is_empty() {
        return Err(AppError::validation("recording label cannot be empty"));
    }

    if label.len() > 256 {
        return Err(AppError::validation("recording label is too long"));
    }

    let mut active_run = state.active_run_id.write().await;

    if let Some(existing) = *active_run {
        return Err(AppError::conflict(format!(
            "recording run {existing} is already active"
        )));
    }

    let run_id = state
        .ptdf
        .start_run(label)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    *active_run = Some(run_id);
    drop(active_run);

    let run = sqlx::query_as::<_, ReplayRunSummary>(
        r#"
        SELECT
            r.id,
            r.label,
            r.started_at,
            r.ended_at,
            0::BIGINT AS event_count,
            r.status,
            TRUE AS active
        FROM ptdf.runs r
        WHERE r.id = $1
        "#,
    )
    .bind(run_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(run))
}

pub async fn get_run(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ReplayRunDetail>, AppError> {
    require_admin(&user).await?;

    let active_run_id = *state.active_run_id.read().await;

    let run = sqlx::query_as::<_, ReplayRunDetail>(
        r#"
        SELECT
            r.id,
            r.label,
            r.started_at,
            r.ended_at,
            r.status,
            COUNT(t.id)::BIGINT AS event_count,
            ($1::UUID IS NOT NULL AND r.id = $1::UUID) AS active
        FROM ptdf.runs r
        LEFT JOIN ptdf.trace_entries t ON t.run_id = r.id
        WHERE r.id = $2
        GROUP BY r.id
        "#,
    )
    .bind(active_run_id)
    .bind(run_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("replay run", run_id.to_string()))?;

    Ok(Json(run))
}

pub async fn stop_run(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ReplayRunSummary>, AppError> {
    require_admin(&user).await?;

    {
        let active_run = state.active_run_id.read().await;
        if *active_run != Some(run_id) {
            return Err(AppError::conflict(format!(
                "replay run {run_id} is not the active recording"
            )));
        }
    }

    state
        .ptdf
        .end_run(run_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let mut active_run = state.active_run_id.write().await;
    if *active_run == Some(run_id) {
        *active_run = None;
    }
    drop(active_run);

    let run = sqlx::query_as::<_, ReplayRunSummary>(
        r#"
        SELECT
            r.id,
            r.label,
            r.started_at,
            r.ended_at,
            r.status,
            COUNT(t.id)::BIGINT AS event_count,
            FALSE AS active
        FROM ptdf.runs r
        LEFT JOIN ptdf.trace_entries t ON t.run_id = r.id
        WHERE r.id = $1
        GROUP BY r.id
        "#,
    )
    .bind(run_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::not_found("replay run", run_id.to_string()))?;

    Ok(Json(run))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReplayEventResponse {
    pub sequence: i64,
    pub event_id: Uuid,
    pub causality_id: Uuid,
    pub origin_service: String,
    pub event_type: String,
    pub recorded_at: DateTime<Utc>,
    pub payload: Value,
}

pub async fn events(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(run_id): Path<Uuid>,
) -> Result<Json<Vec<ReplayEventResponse>>, AppError> {
    require_admin(&user).await?;

    let rows = sqlx::query_as::<_, ReplayEventResponse>(
        r#"
        SELECT
            id AS sequence,
            event_id,
            causality_id,
            origin_service,
            event_type,
            recorded_at,
            payload
        FROM ptdf.trace_entries
        WHERE run_id = $1
        ORDER BY id ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

#[derive(Debug, Serialize)]
pub struct VerificationIssue {
    pub sequence: Option<i64>,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct VerificationResult {
    pub run_id: Uuid,
    pub mode: &'static str,
    pub status: &'static str,
    pub event_count: usize,
    pub issues: Vec<VerificationIssue>,
}

pub async fn verify_run(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(run_id): Path<Uuid>,
) -> Result<Json<VerificationResult>, AppError> {
    require_admin(&user).await?;

    let replayer = PtdfReplayer::new(state.pool.clone());
    let entries = replayer
        .load_run(run_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let mut issues = Vec::new();
    let mut previous_sequence: Option<i64> = None;

    for entry in &entries {
        if let Some(previous) = previous_sequence {
            if entry.sequence <= previous {
                issues.push(VerificationIssue {
                    sequence: Some(entry.sequence),
                    code: "NON_MONOTONIC_SEQUENCE".to_string(),
                    message: format!(
                        "sequence {} does not follow previous sequence {}",
                        entry.sequence, previous
                    ),
                });
            }
        }

        if entry.envelope.event_id != entry.event_id {
            issues.push(VerificationIssue {
                sequence: Some(entry.sequence),
                code: "EVENT_ID_MISMATCH".to_string(),
                message: "stored event_id does not match reconstructed envelope".to_string(),
            });
        }

        if entry.envelope.causality_id != entry.causality_id {
            issues.push(VerificationIssue {
                sequence: Some(entry.sequence),
                code: "CAUSALITY_ID_MISMATCH".to_string(),
                message: "stored causality_id does not match reconstructed envelope".to_string(),
            });
        }

        if entry.envelope.origin_service != entry.origin_service {
            issues.push(VerificationIssue {
                sequence: Some(entry.sequence),
                code: "ORIGIN_SERVICE_MISMATCH".to_string(),
                message: "stored origin_service does not match reconstructed envelope".to_string(),
            });
        }

        if entry.envelope.occurred_at != entry.recorded_at {
            issues.push(VerificationIssue {
                sequence: Some(entry.sequence),
                code: "TIMESTAMP_MISMATCH".to_string(),
                message: "stored recorded_at does not match reconstructed envelope timestamp"
                    .to_string(),
            });
        }

        previous_sequence = Some(entry.sequence);
    }

    Ok(Json(VerificationResult {
        run_id,
        mode: "trace_integrity",
        status: if issues.is_empty() {
            "PASS"
        } else {
            "DIVERGED"
        },
        event_count: entries.len(),
        issues,
    }))
}

#[derive(Debug, Serialize, FromRow)]
pub struct TrainOverview {
    pub id: Uuid,
    pub train_number: String,
    pub operator: Option<String>,
    pub country_code: Option<String>,
    pub service_name: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub speed_kmh: Option<f64>,
    pub bearing: Option<f64>,
    pub current_station: Option<String>,
    pub next_station: Option<String>,
    pub destination: Option<String>,
    pub position_timestamp: Option<DateTime<Utc>>,
    pub source: String,
    pub data_status: String,
    pub passenger_count: Option<i32>,
    pub passenger_capacity: Option<i32>,
    pub occupancy_percentage: Option<f64>,
    pub occupancy_status: Option<String>,
    pub passenger_count_source: Option<String>,
}

pub async fn trains(State(state): State<AppState>) -> Result<Json<Vec<TrainOverview>>, AppError> {
    let rows = sqlx::query_as::<_, TrainOverview>(
        r#"
        SELECT
            id,
            train_number,
            operator,
            country_code::text AS country_code,
            service_name,
            latitude,
            longitude,
            speed_kmh,
            bearing,
            current_station,
            next_station,
            destination,
            position_timestamp,
            source,
            data_status,
            passenger_count,
            passenger_capacity,
            occupancy_percentage,
            occupancy_status,
            passenger_count_source
        FROM tracking.trains
        ORDER BY updated_at DESC, train_number ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

#[derive(Debug, Serialize, FromRow)]
pub struct TrainRoutePoint {
    pub station_id: Uuid,
    pub station_name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub sequence: i32,
}

pub async fn train_route(
    State(state): State<AppState>,
    Path(train_number): Path<String>,
) -> Result<Json<Vec<TrainRoutePoint>>, AppError> {
    let rows = sqlx::query_as::<_, TrainRoutePoint>(
        r#"
        SELECT
            s.id AS station_id,
            s.name AS station_name,
            s.latitude,
            s.longitude,
            tr.sequence
        FROM tracking.train_routes tr
        JOIN tracking.trains t ON t.id = tr.train_id
        JOIN tracking.stations s ON s.id = tr.station_id
        WHERE t.train_number = $1
        ORDER BY tr.sequence ASC
        "#,
    )
    .bind(train_number)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}







