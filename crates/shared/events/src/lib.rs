//! Event envelope: the single structural seam through which PTDF will
//! later intercept every REST request, RabbitMQ message, timer fire, and
//! state transition in the platform.
//!
//! Every service must construct outbound events, publish MQ messages,
//! and record state transitions through `EventEnvelope::wrap`, never by
//! hand-assembling a bare payload. This is what lets PTDF hook in later
//! without any service needing to change its architecture: PTDF only
//! ever needs to observe/intercept at the envelope boundary.

use cbtc_domain::{BlockId, MovementAuthorityId, SignalId, SwitchId, TrainId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generic wrapper around every event/command/message that crosses a
/// service boundary, regardless of transport (HTTP, AMQP, internal timer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<T> {
    pub event_id: Uuid,
    /// Correlates a chain of causally-related events (e.g. a train
    /// position report -> an authority re-evaluation -> a signal
    /// aspect change) even across service boundaries. Propagated by
    /// callers; a fresh `causality_id` is only generated at the true
    /// origin of a causal chain (e.g. an external sensor report).
    pub causality_id: Uuid,
    pub origin_service: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: T,
}

impl<T> EventEnvelope<T> {
    pub fn wrap(origin_service: &'static str, causality_id: Uuid, payload: T) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            causality_id,
            origin_service: origin_service.to_owned(),
            occurred_at: Utc::now(),
            payload,
        }
    }

    /// Start a new causal chain — use only at true origins (external
    /// sensor input, operator command, scheduled timer), never when
    /// reacting to an existing event.
    pub fn originate(origin_service: &'static str, payload: T) -> Self {
        Self::wrap(origin_service, Uuid::new_v4(), payload)
    }

    /// Continue an existing causal chain from a prior envelope of a
    /// (possibly different) payload type.
    pub fn caused_by<U>(
        origin_service: &'static str,
        parent: &EventEnvelope<U>,
        payload: T,
    ) -> Self {
        Self::wrap(origin_service, parent.causality_id, payload)
    }
}

/// The full catalog of domain events published across the platform.
/// RabbitMQ routing keys are derived 1:1 from these variant names by
/// `cbtc-mq`, so adding a variant here is sufficient to make it routable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum DomainEvent {
    // --- train / telemetry ---
    TrainPositionReported {
        train_id: TrainId,
        block_id: BlockId,
        offset_within_block_m: f64,
        speed_kph: f64,
    },
    TrainHeartbeatMissed {
        train_id: TrainId,
        seconds_since_last_heartbeat: i64,
    },
    TrainStateChanged {
        train_id: TrainId,
        previous_state: String,
        new_state: String,
    },

    // --- block occupancy ---
    BlockOccupancyChanged {
        block_id: BlockId,
        previous_state: String,
        new_state: String,
        train_id: Option<TrainId>,
    },

    // --- signaling ---
    SignalAspectChanged {
        signal_id: SignalId,
        previous_aspect: String,
        new_aspect: String,
    },
    SwitchPositionChanged {
        switch_id: SwitchId,
        previous_position: String,
        new_position: String,
    },
    SwitchFaultDetected {
        switch_id: SwitchId,
        description: String,
    },

    // --- movement authority ---
    MovementAuthorityRequested {
        train_id: TrainId,
        requested_blocks: Vec<BlockId>,
    },
    MovementAuthorityGranted {
        authority_id: MovementAuthorityId,
        train_id: TrainId,
        limit_of_movement_m: f64,
    },
    MovementAuthorityRevoked {
        authority_id: MovementAuthorityId,
        train_id: TrainId,
        reason: String,
    },
    MovementAuthorityExpired {
        authority_id: MovementAuthorityId,
        train_id: TrainId,
    },

    // --- safety ---
    EmergencyBrakeTriggered {
        train_id: TrainId,
        reason: String,
    },
    CollisionRiskDetected {
        train_ids: Vec<TrainId>,
        block_id: BlockId,
    },
    /// Raised when a train reports occupying a block that another train
    /// already occupies, per the incumbent's last known state — a
    /// genuine safety anomaly (duplicate/ghost report, sensor fault, or
    /// an actual dual occupancy) that must never be resolved by
    /// silently overwriting `occupied_by`. See
    /// `block-occupancy-service/src/repository.rs::move_train_occupancy`.
    BlockOccupancyConflict {
        block_id: BlockId,
        incumbent_train_id: TrainId,
        reporting_train_id: TrainId,
    },

    // --- failure simulation ---
    SimulatedFaultInjected {
        target_service: String,
        fault_kind: String,
    },

    // --- maintenance ---
    MaintenanceTicketOpened {
        ticket_id: Uuid,
        asset_description: String,
    },

    // --- comms ---
    CommunicationBlackoutStarted {
        train_id: TrainId,
    },
    CommunicationRestored {
        train_id: TrainId,
    },
}

impl DomainEvent {
    /// RabbitMQ routing key for this event, of the form
    /// `cbtc.<domain>.<event>`, e.g. `cbtc.train.position_reported`.
    pub fn routing_key(&self) -> String {
        let variant = match self {
            DomainEvent::TrainPositionReported { .. } => "train.position_reported",
            DomainEvent::TrainHeartbeatMissed { .. } => "train.heartbeat_missed",
            DomainEvent::TrainStateChanged { .. } => "train.state_changed",
            DomainEvent::BlockOccupancyChanged { .. } => "block.occupancy_changed",
            DomainEvent::SignalAspectChanged { .. } => "signal.aspect_changed",
            DomainEvent::SwitchPositionChanged { .. } => "switch.position_changed",
            DomainEvent::SwitchFaultDetected { .. } => "switch.fault_detected",
            DomainEvent::MovementAuthorityRequested { .. } => "authority.requested",
            DomainEvent::MovementAuthorityGranted { .. } => "authority.granted",
            DomainEvent::MovementAuthorityRevoked { .. } => "authority.revoked",
            DomainEvent::MovementAuthorityExpired { .. } => "authority.expired",
            DomainEvent::EmergencyBrakeTriggered { .. } => "safety.emergency_brake",
            DomainEvent::CollisionRiskDetected { .. } => "safety.collision_risk",
            DomainEvent::BlockOccupancyConflict { .. } => "safety.block_occupancy_conflict",
            DomainEvent::SimulatedFaultInjected { .. } => "simulation.fault_injected",
            DomainEvent::MaintenanceTicketOpened { .. } => "maintenance.ticket_opened",
            DomainEvent::CommunicationBlackoutStarted { .. } => "comms.blackout_started",
            DomainEvent::CommunicationRestored { .. } => "comms.restored",
        };
        format!("cbtc.{variant}")
    }
}
