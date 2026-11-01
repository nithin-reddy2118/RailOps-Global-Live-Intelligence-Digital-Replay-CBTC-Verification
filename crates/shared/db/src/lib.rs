//! PostgreSQL connection pool setup and fault-injection wrapper.
//!
//! Every service calls `init_pool` once at startup. The returned pool is
//! a plain `sqlx::PgPool` — fault injection (latency, occasional forced
//! errors) is applied by wrapping individual query call sites with
//! `with_fault_injection`, not by wrapping the pool itself, so normal
//! `sqlx::query!` macro usage in services is unaffected when fault
//! injection is disabled (the default).

use cbtc_config::{DatabaseConfig, FaultInjectionConfig};
use cbtc_errors::{AppError, AppResult};
use rand::Rng;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

/// Initialize the connection pool and run pending migrations from the
/// `migrations/` directory at the workspace root. Every service embeds
/// migrations via `sqlx::migrate!("../../../migrations")` relative to
/// its own crate so a single migration set is shared across all services
/// that touch the database directly.
pub async fn init_pool(cfg: &DatabaseConfig) -> AppResult<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&cfg.url)
        .await?;

    tracing::info!(
        max_connections = cfg.max_connections,
        "database pool initialized"
    );
    Ok(pool)
}

/// Wrap a fallible database operation with configured fault injection.
/// When `fault_injection.enabled` is false this is a zero-cost passthrough.
///
/// Usage:
/// ```ignore
/// let train = with_fault_injection(&fault_cfg, || async {
///     sqlx::query_as!(Train, "SELECT * FROM train.trains WHERE id = $1", id)
///         .fetch_one(&pool)
///         .await
///         .map_err(AppError::from)
/// }).await?;
/// ```
pub async fn with_fault_injection<T, F, Fut>(cfg: &FaultInjectionConfig, op: F) -> AppResult<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = AppResult<T>>,
{
    if !cfg.enabled {
        return op().await;
    }

    if cfg.latency_ms_max > 0 {
        let delay = {
            let mut rng = rand::thread_rng();
            let min = cfg.latency_ms_min.min(cfg.latency_ms_max);
            rng.gen_range(min..=cfg.latency_ms_max)
        };
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    }

    if cfg.packet_loss_probability > 0.0 {
        let roll: f64 = rand::thread_rng().gen();
        if roll < cfg.packet_loss_probability {
            tracing::warn!("fault injection: simulated database packet loss");
            return Err(AppError::Database(sqlx::Error::PoolTimedOut));
        }
    }

    op().await
}

/// Simple health check used by readiness probes.
pub async fn health_check(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}
