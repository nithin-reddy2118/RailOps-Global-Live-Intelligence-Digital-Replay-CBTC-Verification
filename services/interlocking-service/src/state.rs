use crate::topology_cache::TopologyCache;
use cbtc_auth::JwtCodec;
use cbtc_mq::MqClient;
use sqlx::PgPool;
use std::sync::Arc;

/// Base URLs for the peer services interlocking-service calls into.
/// Interlocking never writes directly to `train.movement_authorities` —
/// it always goes through movement-authority-service's HTTP API so that
/// service remains the single writer responsible for the
/// one-live-authority-per-train invariant.
#[derive(Clone)]
pub struct PeerServices {
    pub movement_authority_base_url: String,
    pub service_token: String,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_codec: Arc<JwtCodec>,
    pub mq: Arc<MqClient>,
    pub http: reqwest::Client,
    pub peers: PeerServices,
    pub topology: Arc<TopologyCache>,
}
