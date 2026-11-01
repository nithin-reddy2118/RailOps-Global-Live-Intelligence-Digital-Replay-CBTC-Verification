use crate::{
    models::{EmergencyStopResult, NetworkOverview},
    repository::OverviewRepository,
    state::AppState,
};
use axum::{extract::State, Json};
use cbtc_auth::{AuthenticatedUser, Role};
use cbtc_errors::{AppError, AppResult};
use chrono::Utc;
use serde::Serialize;

pub async fn overview(State(state): State<AppState>) -> AppResult<Json<NetworkOverview>> {
    let repo = OverviewRepository::new(&state.pool);

    // These six reads are independent and could race safely against a
    // concurrently-changing network; there's no cross-read consistency
    // requirement here (unlike the safety checks in interlocking-service)
    // since this endpoint feeds a dashboard display, not a control
    // decision — a one-request-old value briefly shown is harmless.
    let (trains, blocks, signals, switches, active_authorities, unacknowledged_alerts) = tokio::try_join!(
        repo.trains(),
        repo.blocks(),
        repo.signals(),
        repo.switches(),
        repo.active_authorities(),
        repo.unacknowledged_alerts(),
    )?;

    Ok(Json(NetworkOverview {
        trains,
        blocks,
        signals,
        switches,
        active_authorities,
        unacknowledged_alerts,
        generated_at: Utc::now(),
    }))
}

#[derive(Serialize)]
struct RevokeBody<'a> {
    reason: &'a str,
}

/// `POST /control/emergency-stop-all` — admin-only, platform-wide
/// emergency stop. Revokes every currently live movement authority via
/// movement-authority-service (never writes directly to its table),
/// individually, tolerating and reporting per-authority failures rather
/// than aborting partway — an emergency stop must make its best effort
/// against every train even if one revoke call fails.
pub async fn emergency_stop_all(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<EmergencyStopResult>> {
    auth_user.require_role(Role::Admin)?;

    let repo = OverviewRepository::new(&state.pool);
    let authority_ids = repo.active_authority_ids().await?;

    let mut revoked = 0usize;
    let mut failures = Vec::new();

    for id in authority_ids {
        let url = format!(
            "{}/authorities/{}/revoke",
            state.peers.movement_authority_base_url, id
        );
        let result = state
            .http
            .post(&url)
            .bearer_auth(&state.peers.service_token)
            .json(&RevokeBody {
                reason: "platform-wide emergency stop",
            })
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => revoked += 1,
            Ok(resp) => failures.push(format!(
                "authority {id}: upstream returned {}",
                resp.status()
            )),
            Err(e) => failures.push(format!("authority {id}: {e}")),
        }
    }

    tracing::error!(
        revoked,
        failures = failures.len(),
        "EMERGENCY STOP ALL executed by admin {}",
        auth_user.user_id
    );

    if revoked == 0 && !failures.is_empty() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "emergency stop failed for all authorities"
        )));
    }

    Ok(Json(EmergencyStopResult {
        authorities_revoked: revoked,
        failures,
    }))
}
