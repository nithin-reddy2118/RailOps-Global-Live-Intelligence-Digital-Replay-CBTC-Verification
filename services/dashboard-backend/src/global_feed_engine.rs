use anyhow::Result;
use cbtc_ptdf::PtdfRecorder;
use serde_json::json;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::gtfs_rt_connector::GtfsRtConnector;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GlobalFeed {
    pub id: Uuid,
    pub country_code: String,
    pub operator: String,
    pub feed_name: String,
    pub vehicle_positions_url: Option<String>,
    pub trip_updates_url: Option<String>,
    pub feed_format: String,
    pub enabled: bool,
    pub refresh_seconds: i32,
}

pub async fn load_enabled_feeds(pool: &PgPool) -> Result<Vec<GlobalFeed>> {
    Ok(sqlx::query_as::<_, GlobalFeed>(
        r#"
        SELECT id, country_code, operator, feed_name,
               vehicle_positions_url, trip_updates_url,
               feed_format, enabled, refresh_seconds
        FROM tracking.global_feed_registry
        WHERE enabled = TRUE
        ORDER BY country_code, operator, feed_name
        "#,
    )
    .fetch_all(pool)
    .await?)
}

pub async fn run_global_feed_engine(
    pool: PgPool,
    ptdf: Arc<PtdfRecorder>,
    run_id: Option<Uuid>,
) -> Result<()> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("global-cbtc-platform/0.1")
        .build()?;

    let mut workers: HashMap<Uuid, JoinHandle<()>> = HashMap::new();

    loop {
        let feeds = match load_enabled_feeds(&pool).await {
            Ok(value) => value,
            Err(error) => {
                tracing::error!(error = %error, "failed to load global feed registry");

                if let Some(active_run_id) = run_id {
                    let _ = ptdf
                        .record_platform_event(
                            active_run_id,
                            "platform.feed_registry.load_error",
                            "dashboard-backend",
                            json!({
                                "error": error.to_string()
                            }),
                        )
                        .await;
                }

                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let enabled: HashSet<Uuid> = feeds.iter().map(|f| f.id).collect();

        workers.retain(|id, handle| {
            if enabled.contains(id) {
                true
            } else {
                handle.abort();
                tracing::info!(feed_id = %id, "stopped disabled global feed worker");
                false
            }
        });

        for feed in feeds {
            if workers.contains_key(&feed.id) {
                continue;
            }

            let worker_pool = pool.clone();
            let worker_http = http.clone();
            let worker_ptdf = ptdf.clone();
            let worker_run_id = run_id;
            let id = feed.id;

            workers.insert(
                id,
                tokio::spawn(async move {
                    feed_worker(
                        worker_pool,
                        worker_http,
                        worker_ptdf,
                        worker_run_id,
                        feed,
                    )
                    .await;
                }),
            );
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn feed_worker(
    pool: PgPool,
    http: reqwest::Client,
    ptdf: Arc<PtdfRecorder>,
    run_id: Option<Uuid>,
    feed: GlobalFeed,
) {
    let refresh = feed.refresh_seconds.max(5) as u64;

    tracing::info!(
        feed = %feed.feed_name,
        country = %feed.country_code,
        operator = %feed.operator,
        refresh_seconds = refresh,
        "started global railway feed worker"
    );

    if let Some(active_run_id) = run_id {
        if let Err(error) = ptdf
            .record_platform_event(
                active_run_id,
                "platform.feed.worker_started",
                "dashboard-backend",
                json!({
                    "feed_id": feed.id,
                    "feed_name": feed.feed_name,
                    "operator": feed.operator,
                    "country_code": feed.country_code,
                    "refresh_seconds": refresh
                }),
            )
            .await
        {
            tracing::warn!(%error, "failed to record feed worker start in PTDF");
        }
    }

    loop {
        let started = std::time::Instant::now();

        match synchronize_feed(&pool, &http, &ptdf, run_id, &feed).await {
            Ok(count) => {
                let elapsed_ms = started.elapsed().as_millis() as u64;

                let _ = sqlx::query(
                    r#"
                    UPDATE tracking.global_feed_registry
                    SET last_attempt_at = NOW(),
                        last_success_at = NOW(),
                        last_error = NULL,
                        consecutive_failures = 0,
                        updated_at = NOW()
                    WHERE id = $1
                    "#,
                )
                .bind(feed.id)
                .execute(&pool)
                .await;

                if let Some(active_run_id) = run_id {
                    if let Err(error) = ptdf
                        .record_platform_event(
                            active_run_id,
                            "platform.feed.synchronized",
                            "dashboard-backend",
                            json!({
                                "feed_id": feed.id,
                                "feed_name": feed.feed_name,
                                "operator": feed.operator,
                                "country_code": feed.country_code,
                                "train_count": count,
                                "elapsed_ms": elapsed_ms,
                                "status": "success"
                            }),
                        )
                        .await
                    {
                        tracing::warn!(%error, "failed to record feed synchronization in PTDF");
                    }
                }

                tracing::info!(
                    feed = %feed.feed_name,
                    trains = count,
                    elapsed_ms = elapsed_ms,
                    "global railway feed synchronized"
                );
            }
            Err(error) => {
                let elapsed_ms = started.elapsed().as_millis() as u64;

                tracing::error!(
                    feed = %feed.feed_name,
                    error = %error,
                    "global railway feed synchronization failed"
                );

                let _ = sqlx::query(
                    r#"
                    UPDATE tracking.global_feed_registry
                    SET last_attempt_at = NOW(),
                        last_error = $2,
                        consecutive_failures = consecutive_failures + 1,
                        updated_at = NOW()
                    WHERE id = $1
                    "#,
                )
                .bind(feed.id)
                .bind(error.to_string())
                .execute(&pool)
                .await;

                if let Some(active_run_id) = run_id {
                    if let Err(record_error) = ptdf
                        .record_platform_event(
                            active_run_id,
                            "platform.feed.synchronization_error",
                            "dashboard-backend",
                            json!({
                                "feed_id": feed.id,
                                "feed_name": feed.feed_name,
                                "operator": feed.operator,
                                "country_code": feed.country_code,
                                "error": error.to_string(),
                                "elapsed_ms": elapsed_ms,
                                "status": "error"
                            }),
                        )
                        .await
                    {
                        tracing::warn!(
                            %record_error,
                            "failed to record feed synchronization error in PTDF"
                        );
                    }
                }
            }
        }

        let stale_seconds = (refresh * 3).max(120) as i64;

        let _ = sqlx::query(
            r#"
            UPDATE tracking.trains
            SET data_status = 'STALE',
                updated_at = NOW()
            WHERE source = $1
              AND data_status = 'LIVE'
              AND COALESCE(position_timestamp, updated_at) < NOW() - ($2 * INTERVAL '1 second')
            "#,
        )
        .bind(format!("gtfs_rt:{}", feed.feed_name))
        .bind(stale_seconds)
        .execute(&pool)
        .await;

        tokio::time::sleep(Duration::from_secs(refresh)).await;
    }
}

async fn synchronize_feed(
    pool: &PgPool,
    http: &reqwest::Client,
    ptdf: &Arc<PtdfRecorder>,
    run_id: Option<Uuid>,
    feed: &GlobalFeed,
) -> Result<usize> {
    if !feed.feed_format.eq_ignore_ascii_case("GTFS_RT") {
        anyhow::bail!("unsupported feed format '{}'", feed.feed_format);
    }

    let vehicle_url = feed
        .vehicle_positions_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("vehicle positions URL is missing"))?;

    let connector = GtfsRtConnector::new(
        http.clone(),
        vehicle_url.to_string(),
        feed.trip_updates_url.clone(),
        feed.operator.clone(),
        feed.country_code.clone(),
        feed.feed_name.clone(),
    );

    let trains = connector.fetch_trains().await?;
    let count = trains.len();

    if let Some(active_run_id) = run_id {
        let snapshot = trains
            .iter()
            .map(|train| {
                json!({
                    "id": train.id,
                    "train_number": train.train_number,
                    "operator": train.operator,
                    "country_code": train.country_code,
                    "latitude": train.latitude,
                    "longitude": train.longitude,
                    "speed_kmh": train.speed_kmh,
                    "bearing": train.bearing,
                    "position_timestamp": train.position_timestamp,
                    "data_status": train.data_status
                })
            })
            .collect::<Vec<_>>();

        if let Err(error) = ptdf
            .record_platform_event(
                active_run_id,
                "platform.train_positions.snapshot",
                "dashboard-backend",
                json!({
                    "feed_id": feed.id,
                    "feed_name": feed.feed_name,
                    "operator": feed.operator,
                    "country_code": feed.country_code,
                    "train_count": count,
                    "trains": snapshot
                }),
            )
            .await
        {
            tracing::warn!(
                %error,
                feed = %feed.feed_name,
                "failed to record train position snapshot in PTDF"
            );
        }
    }

    for train in trains {
        sqlx::query(
            r#"
            INSERT INTO tracking.trains
                (id, train_number, operator, country_code, service_name, latitude, longitude, speed_kmh, bearing, current_station, next_station, destination, position_timestamp, source, data_status, passenger_count, passenger_capacity, occupancy_percentage, occupancy_status, passenger_count_source)
            VALUES
                ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20)
            ON CONFLICT (id) DO UPDATE SET
                train_number = EXCLUDED.train_number,
                operator = EXCLUDED.operator,
                country_code = EXCLUDED.country_code,
                service_name = EXCLUDED.service_name,
                latitude = EXCLUDED.latitude,
                longitude = EXCLUDED.longitude,
                speed_kmh = EXCLUDED.speed_kmh,
                bearing = EXCLUDED.bearing,
                current_station = EXCLUDED.current_station,
                next_station = EXCLUDED.next_station,
                destination = EXCLUDED.destination,
                position_timestamp = EXCLUDED.position_timestamp,
                source = EXCLUDED.source,
                data_status = EXCLUDED.data_status,
                passenger_count = COALESCE(EXCLUDED.passenger_count, tracking.trains.passenger_count),
                passenger_capacity = COALESCE(EXCLUDED.passenger_capacity, tracking.trains.passenger_capacity),
                occupancy_percentage = COALESCE(EXCLUDED.occupancy_percentage, tracking.trains.occupancy_percentage),
                occupancy_status = COALESCE(EXCLUDED.occupancy_status, tracking.trains.occupancy_status),
                passenger_count_source = COALESCE(EXCLUDED.passenger_count_source, tracking.trains.passenger_count_source),
                updated_at = NOW()
            "#,
        )
        .bind(train.id)
        .bind(&train.train_number)
        .bind(&train.operator)
        .bind(&train.country_code)
        .bind(&train.service_name)
        .bind(train.latitude)
        .bind(train.longitude)
        .bind(train.speed_kmh)
        .bind(train.bearing)
        .bind(&train.current_station)
        .bind(&train.next_station)
        .bind(&train.destination)
        .bind(train.position_timestamp)
        .bind(&train.source)
        .bind(&train.data_status)
        .bind(train.passenger_count)
        .bind(train.passenger_capacity)
        .bind(train.occupancy_percentage)
        .bind(&train.occupancy_status)
        .bind(&train.passenger_count_source)
        .execute(pool)
        .await?;
    }

    Ok(count)
}
