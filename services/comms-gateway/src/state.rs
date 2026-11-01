use cbtc_auth::JwtCodec;
use cbtc_config::FaultInjectionConfig;
use cbtc_mq::MqClient;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// Trains currently in a simulated communication blackout. Held
/// in-memory rather than in Postgres/Redis: blackout state is a
/// transient simulation control, not durable operational data, and a
/// gateway restart clearing it (trains simply "reconnect") is the
/// correct behavior for a demo failure-injection tool, not a bug.
#[derive(Default)]
pub struct BlackoutRegistry {
    inner: RwLock<HashSet<Uuid>>,
}

impl BlackoutRegistry {
    pub fn set(&self, train_id: Uuid, in_blackout: bool) {
        let mut set = self.inner.write().expect("blackout registry lock poisoned");
        if in_blackout {
            set.insert(train_id);
        } else {
            set.remove(&train_id);
        }
    }

    pub fn is_in_blackout(&self, train_id: Uuid) -> bool {
        self.inner
            .read()
            .expect("blackout registry lock poisoned")
            .contains(&train_id)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub jwt_codec: Arc<JwtCodec>,
    pub mq: Arc<MqClient>,
    pub http: reqwest::Client,
    pub train_service_base_url: String,
    pub fault_injection: FaultInjectionConfig,
    pub blackouts: Arc<BlackoutRegistry>,
}
