use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gtfs_rt::FeedMessage;
use prost::Message;
use reqwest::Client;
use std::collections::HashMap;
use uuid::Uuid;

use crate::global_data::{NormalizedStation, NormalizedTrain, RailwayDataConnector};

pub struct GtfsRtConnector {
    pub client: Client,
    pub vehicle_positions_url: String,
    pub trip_updates_url: Option<String>,
    pub operator: String,
    pub country_code: String,
    pub feed_name: String,
}

impl GtfsRtConnector {
    pub fn new(
        client: Client,
        vehicle_positions_url: impl Into<String>,
        trip_updates_url: Option<String>,
        operator: impl Into<String>,
        country_code: impl Into<String>,
        feed_name: impl Into<String>,
    ) -> Self {
        Self {
            client,
            vehicle_positions_url: vehicle_positions_url.into(),
            trip_updates_url,
            operator: operator.into(),
            country_code: country_code.into(),
            feed_name: feed_name.into(),
        }
    }

    async fn fetch_feed(&self, url: &str) -> Result<FeedMessage> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", "global-cbtc-platform/0.1")
            .send()
            .await
            .context("failed to request GTFS-RT feed")?
            .error_for_status()
            .context("GTFS-RT returned an HTTP error")?;

        let bytes = response
            .bytes()
            .await
            .context("failed to read GTFS-RT response")?;

        FeedMessage::decode(bytes.as_ref())
            .context("failed to decode GTFS-RT protobuf")
    }

    fn trip_update_index(
        feed: &FeedMessage,
    ) -> HashMap<String, gtfs_rt::TripUpdate> {
        let mut updates = HashMap::new();

        for entity in &feed.entity {
            if let Some(update) = entity.trip_update.as_ref() {
                if let Some(trip_id) = update.trip.trip_id.clone() {
                    updates.insert(trip_id, update.clone());
                }
            }
        }

        updates
    }

    fn entity_to_train(
        &self,
        entity: &gtfs_rt::FeedEntity,
        feed_timestamp: Option<u64>,
        trip_updates: &HashMap<String, gtfs_rt::TripUpdate>,
    ) -> Option<NormalizedTrain> {
        let position = entity.vehicle.as_ref()?;
        let vehicle_position = position.position.as_ref()?;

        let latitude = vehicle_position.latitude as f64;
        let longitude = vehicle_position.longitude as f64;

        if !latitude.is_finite()
            || !longitude.is_finite()
            || !(-90.0..=90.0).contains(&latitude)
            || !(-180.0..=180.0).contains(&longitude)
        {
            return None;
        }

        let vehicle_id = position
            .vehicle
            .as_ref()
            .and_then(|vehicle| vehicle.id.clone())
            .or_else(|| {
                position
                    .vehicle
                    .as_ref()
                    .and_then(|vehicle| vehicle.label.clone())
            })
            .unwrap_or_else(|| entity.id.clone());

        let trip_id = position
            .trip
            .as_ref()
            .and_then(|trip| trip.trip_id.clone());

        let trip_update = trip_id
            .as_ref()
            .and_then(|id| trip_updates.get(id).cloned());

        let route_id = trip_update.as_ref().and_then(|update| update.trip.route_id.clone());

        let train_number = position
            .vehicle
            .as_ref()
            .and_then(|vehicle| vehicle.label.clone())
            .unwrap_or_else(|| vehicle_id.clone());

        let service_name = route_id
            .clone()
            .or_else(|| trip_id.clone())
            .unwrap_or_else(|| vehicle_id.clone());

        let service_text = format!("{} {} {}", train_number, service_name, vehicle_id).to_ascii_lowercase();
        if ["boat", "ferry", "bus", "shuttle"].iter().any(|value| service_text.contains(value)) {
            return None;
        }

        let timestamp = position
            .timestamp
            .or(feed_timestamp)
            .and_then(|seconds| DateTime::from_timestamp(seconds as i64, 0));

        let speed_kmh = vehicle_position
            .speed
            .map(|value| value as f64 * 3.6);

        let bearing = vehicle_position.bearing.map(|value| value as f64);

        let mut current_station = None;
        let mut next_station = None;

        if let Some(update) = trip_update {
            let now = Utc::now().timestamp();
            let mut previous_stop = None;

            for stop in &update.stop_time_update {
                let stop_id = stop.stop_id.clone();

                let event_timestamp = stop
                    .arrival
                    .as_ref()
                    .and_then(|event| event.time)
                    .or_else(|| {
                        stop.departure
                            .as_ref()
                            .and_then(|event| event.time)
                    });

                match (stop_id, event_timestamp) {
                    (Some(stop_id), Some(timestamp)) if timestamp <= now => {
                        previous_stop = Some(stop_id);
                    }
                    (Some(stop_id), Some(_)) => {
                        next_station = Some(stop_id);
                        current_station = previous_stop.clone();
                        break;
                    }
                    _ => {}
                }
            }
        }

        let stable_key = format!(
            "gtfsrt:{}:{}:{}",
            self.country_code, self.operator, vehicle_id
        );

        Some(NormalizedTrain {
            id: Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                stable_key.as_bytes(),
            ),
            train_number,
            operator: self.operator.clone(),
            country_code: self.country_code.clone(),
            service_name,
            latitude: Some(latitude),
            longitude: Some(longitude),
            speed_kmh,
            bearing,
            current_station,
            next_station,
            destination: None,
            position_timestamp: timestamp,
            source: format!("gtfs_rt:{}", self.feed_name),
            data_status: "LIVE".to_string(),
            passenger_count: None,
            passenger_capacity: None,
            occupancy_percentage: None,
            occupancy_status: None,
            passenger_count_source: None,
        })
    }
}

#[async_trait]
impl RailwayDataConnector for GtfsRtConnector {
    async fn fetch_trains(&self) -> Result<Vec<NormalizedTrain>> {
        let vehicle_feed = self
            .fetch_feed(&self.vehicle_positions_url)
            .await?;

        let trip_updates = if let Some(url) = &self.trip_updates_url {
            let trip_feed = self.fetch_feed(url).await?;
            Self::trip_update_index(&trip_feed)
        } else {
            HashMap::new()
        };

        let feed_timestamp = vehicle_feed.header.timestamp;

        Ok(vehicle_feed
            .entity
            .iter()
            .filter_map(|entity| {
                self.entity_to_train(
                    entity,
                    feed_timestamp,
                    &trip_updates,
                )
            })
            .collect())
    }

    async fn fetch_stations(&self) -> Result<Vec<NormalizedStation>> {
        Ok(Vec::new())
    }
}




