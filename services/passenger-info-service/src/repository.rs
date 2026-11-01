use crate::models::{PlatformStatus, PlatformSummary, StationResponse};
use cbtc_errors::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

pub struct PassengerInfoRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> PassengerInfoRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_stations(&self) -> AppResult<Vec<StationResponse>> {
        let stations: Vec<(Uuid, String, f64)> = sqlx::query_as(
            "SELECT id, name, chainage_m FROM signaling.stations ORDER BY chainage_m",
        )
        .fetch_all(self.pool)
        .await?;

        let mut result = Vec::with_capacity(stations.len());
        for (id, name, chainage_m) in stations {
            let platforms: Vec<(Uuid, String, Uuid, i32)> = sqlx::query_as(
                "SELECT id, name, adjacent_block, dwell_time_default_secs FROM signaling.platforms WHERE station_id = $1",
            )
            .bind(id)
            .fetch_all(self.pool)
            .await?;

            result.push(StationResponse {
                id,
                name,
                chainage_m,
                platforms: platforms
                    .into_iter()
                    .map(|(pid, pname, block, dwell)| PlatformSummary {
                        id: pid,
                        name: pname,
                        adjacent_block: block,
                        dwell_time_default_secs: dwell,
                    })
                    .collect(),
            });
        }

        Ok(result)
    }

    pub async fn platform_status(&self, platform_id: Uuid) -> AppResult<PlatformStatus> {
        let adjacent_block: Option<(Uuid,)> =
            sqlx::query_as("SELECT adjacent_block FROM signaling.platforms WHERE id = $1")
                .bind(platform_id)
                .fetch_optional(self.pool)
                .await?;

        let (adjacent_block,) = adjacent_block
            .ok_or_else(|| AppError::not_found("platform", platform_id.to_string()))?;

        // Case 1: a train is directly occupying the platform's block.
        let at_platform: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT t.id, t.identifier
            FROM signaling.blocks b
            JOIN train.trains t ON t.id = b.occupied_by
            WHERE b.id = $1 AND b.occupancy = 'occupied'
            "#,
        )
        .bind(adjacent_block)
        .fetch_optional(self.pool)
        .await?;

        if let Some((train_id, identifier)) = at_platform {
            return Ok(PlatformStatus::TrainAtPlatform {
                train_id,
                train_identifier: identifier,
            });
        }

        // Case 2: a train holds a live movement authority whose block
        // sequence terminates at (or passes through) this platform's
        // block — it's approaching. ETA is estimated from its most
        // recent position report; `NULL` if no recent report exists or
        // the train has stopped (speed ~ 0), since a distance/speed
        // estimate is meaningless in that case.
        #[allow(clippy::type_complexity)]
        let approaching: Option<(Uuid, String, Option<f64>, Option<f64>, Option<f64>)> =
            sqlx::query_as(
                r#"
            SELECT
                t.id,
                t.identifier,
                pr.offset_within_block_m,
                pr.speed_kph,
                b.length_m
            FROM train.movement_authorities ma
            JOIN train.trains t ON t.id = ma.train_id
            JOIN signaling.blocks b ON b.id = t.current_block
            LEFT JOIN LATERAL (
                SELECT offset_within_block_m, speed_kph
                FROM train.position_reports
                WHERE train_id = t.id
                ORDER BY reported_at DESC
                LIMIT 1
            ) pr ON TRUE
            WHERE ma.status IN ('granted', 'active')
              AND $1 = ANY(ma.block_sequence)
            LIMIT 1
            "#,
            )
            .bind(adjacent_block)
            .fetch_optional(self.pool)
            .await?;

        if let Some((train_id, identifier, offset, speed, block_length)) = approaching {
            let eta = match (offset, speed, block_length) {
                (Some(offset), Some(speed_kph), Some(length)) if speed_kph > 1.0 => {
                    let remaining_m = (length - offset).max(0.0);
                    let speed_mps = speed_kph * 1000.0 / 3600.0;
                    Some(remaining_m / speed_mps)
                }
                _ => None,
            };
            return Ok(PlatformStatus::TrainApproaching {
                train_id,
                train_identifier: identifier,
                estimated_seconds_away: eta,
            });
        }

        Ok(PlatformStatus::NoTrainExpected)
    }
}
