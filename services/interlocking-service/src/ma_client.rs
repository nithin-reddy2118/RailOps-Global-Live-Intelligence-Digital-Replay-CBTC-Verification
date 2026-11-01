use crate::state::PeerServices;
use cbtc_domain::{BlockId, MovementAuthorityId, TrainId};
use cbtc_errors::AppError;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct GrantRequestBody {
    train_id: TrainId,
    block_sequence: Vec<BlockId>,
    limit_of_movement_m: f64,
    max_authorized_speed_kph: i32,
    ttl_secs: i64,
}

#[derive(Deserialize)]
pub struct GrantedAuthority {
    pub id: MovementAuthorityId,
}

/// Distinguishes "movement-authority-service rejected the grant because
/// another train already holds an overlapping authority" from any other
/// upstream failure, so the caller can surface a structured
/// `RequestAuthorityResponse::Rejected` instead of a generic 502.
pub enum GrantOutcome {
    Granted(GrantedAuthority),
    Conflict(String),
}

/// Calls `POST /authorities` on movement-authority-service. Interlocking
/// authenticates as a service account (a long-lived operator-role token
/// configured via `INTERLOCKING_SERVICE_TOKEN`) rather than forwarding
/// any end-user's token, since a route computed by interlocking is
/// granted on interlocking's own authority, independent of which human
/// (if any) triggered the original request.
pub async fn grant_authority(
    http: &reqwest::Client,
    peers: &PeerServices,
    train_id: TrainId,
    block_sequence: Vec<BlockId>,
    limit_of_movement_m: f64,
    max_authorized_speed_kph: i32,
    ttl_secs: i64,
) -> Result<GrantOutcome, AppError> {
    let url = format!("{}/authorities", peers.movement_authority_base_url);
    let body = GrantRequestBody {
        train_id,
        block_sequence,
        limit_of_movement_m,
        max_authorized_speed_kph,
        ttl_secs,
    };

    let response = http
        .post(&url)
        .bearer_auth(&peers.service_token)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::UpstreamService {
            service: "movement-authority-service",
            status: e.status().map(|s| s.as_u16()).unwrap_or(0),
        })?;

    if response.status().as_u16() == 409 {
        let body_text = response.text().await.unwrap_or_default();
        return Ok(GrantOutcome::Conflict(body_text));
    }

    if !response.status().is_success() {
        return Err(AppError::UpstreamService {
            service: "movement-authority-service",
            status: response.status().as_u16(),
        });
    }

    let granted =
        response
            .json::<GrantedAuthority>()
            .await
            .map_err(|_| AppError::UpstreamService {
                service: "movement-authority-service",
                status: 200,
            })?;
    Ok(GrantOutcome::Granted(granted))
}

/// Calls `POST /authorities/:id/revoke`.
pub async fn revoke_authority(
    http: &reqwest::Client,
    peers: &PeerServices,
    authority_id: MovementAuthorityId,
    reason: &str,
) -> Result<(), AppError> {
    let url = format!(
        "{}/authorities/{}/revoke",
        peers.movement_authority_base_url, authority_id
    );

    #[derive(Serialize)]
    struct RevokeBody<'a> {
        reason: &'a str,
    }

    let response = http
        .post(&url)
        .bearer_auth(&peers.service_token)
        .json(&RevokeBody { reason })
        .send()
        .await
        .map_err(|e| AppError::UpstreamService {
            service: "movement-authority-service",
            status: e.status().map(|s| s.as_u16()).unwrap_or(0),
        })?;

    if !response.status().is_success() {
        return Err(AppError::UpstreamService {
            service: "movement-authority-service",
            status: response.status().as_u16(),
        });
    }

    Ok(())
}
