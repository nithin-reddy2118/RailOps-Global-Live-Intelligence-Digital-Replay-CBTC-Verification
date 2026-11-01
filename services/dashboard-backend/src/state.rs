use cbtc_ptdf::PtdfRecorder;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

const BROADCAST_CAPACITY: usize = 1024;

#[derive(Clone)]
pub struct AppState {
    pub event_tx: broadcast::Sender<String>,
    pub http: reqwest::Client,
    pub central_control_base_url: String,
    pub pool: PgPool,
    pub ptdf: Arc<PtdfRecorder>,
    pub active_run_id: Arc<RwLock<Option<Uuid>>>,
}

impl AppState {
    pub fn new(
        http: reqwest::Client,
        central_control_base_url: String,
        pool: PgPool,
        ptdf: Arc<PtdfRecorder>,
        active_run_id: Option<Uuid>,
    ) -> Self {
        let (event_tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            event_tx,
            http,
            central_control_base_url,
            pool,
            ptdf,
            active_run_id: Arc::new(RwLock::new(active_run_id)),
        }
    }
}
