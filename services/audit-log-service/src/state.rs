use cbtc_auth::JwtCodec;
use cbtc_mq::MqClient;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_codec: Arc<JwtCodec>,
    pub mq: Arc<MqClient>,
}
