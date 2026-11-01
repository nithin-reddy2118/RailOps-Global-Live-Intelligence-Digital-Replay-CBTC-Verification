//! Newtype identifiers.
//!
//! Every entity ID in the platform is a distinct type wrapping `Uuid`
//! rather than a bare `Uuid`, so the compiler rejects e.g. passing a
//! `SignalId` where a `TrainId` is expected. This has caught real bugs
//! in signaling-adjacent systems in the wild; it's cheap insurance here.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
        #[sqlx(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

define_id!(TrainId);
define_id!(BlockId);
define_id!(SignalId);
define_id!(SwitchId);
define_id!(StationId);
define_id!(PlatformId);
define_id!(RouteId);
define_id!(MovementAuthorityId);
define_id!(WaysideUnitId);
define_id!(UserId);
define_id!(MaintenanceTicketId);
define_id!(TrackSegmentId);
define_id!(AlertId);
