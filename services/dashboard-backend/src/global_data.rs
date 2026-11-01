use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;
#[derive(Debug, Clone)]
pub struct NormalizedTrain {
    pub id: Uuid,
    pub train_number: String,
    pub operator: String,
    pub country_code: String,
    pub service_name: String,
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

#[derive(Debug, Clone)]
pub struct NormalizedStation {
    pub id: Uuid,
    pub name: String,
    pub country_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub station_code: Option<String>,
    pub operator: String,
    pub source: String,
}

#[async_trait]
pub trait RailwayDataConnector: Send + Sync {
    async fn fetch_trains(&self) -> Result<Vec<NormalizedTrain>>;
    async fn fetch_stations(&self) -> Result<Vec<NormalizedStation>>;
}

pub async fn sync_connector<C: RailwayDataConnector>(pool: &PgPool, connector: &C) -> Result<()> {
    for station in connector.fetch_stations().await? {
        sqlx::query(
            r#"
            INSERT INTO tracking.stations
                (id, name, country_code, latitude, longitude, station_code, operator, source)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                country_code = EXCLUDED.country_code,

                latitude = EXCLUDED.latitude,
                longitude = EXCLUDED.longitude,
                station_code = EXCLUDED.station_code,
                operator = EXCLUDED.operator,
                source = EXCLUDED.source
            "#,
        )
        .bind(station.id)
        .bind(&station.name)
        .bind(&station.country_code)
        .bind(station.latitude)
        .bind(station.longitude)
        .bind(&station.station_code)
        .bind(&station.operator)
        .bind(&station.source)
        .execute(pool)
        .await?;
    }

    for train in connector.fetch_trains().await? {
        sqlx::query(
            r#"
            INSERT INTO tracking.trains
                (id, train_number, operator, country_code, service_name,
                 latitude, longitude, speed_kmh, bearing, current_station,
                 next_station, destination, position_timestamp, source, data_status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
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

    Ok(())
}


