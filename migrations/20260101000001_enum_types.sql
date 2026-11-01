-- Enum types backing the sqlx::Type derives in cbtc-domain.
-- Kept in one migration since every subsequent table references them.

CREATE TYPE occupancy_state AS ENUM ('clear', 'occupied', 'ambiguous', 'out_of_service');
CREATE TYPE signal_aspect AS ENUM ('red', 'yellow_single', 'yellow_double', 'green', 'failed', 'unknown');
CREATE TYPE signal_kind AS ENUM ('main', 'shunt', 'repeater', 'distant');
CREATE TYPE switch_position AS ENUM ('normal', 'reverse', 'in_transition', 'failed');
CREATE TYPE train_operational_state AS ENUM (
    'out_of_service', 'staged', 'in_motion', 'stopped', 'emergency_braking', 'degraded', 'communication_lost'
);
CREATE TYPE authority_status AS ENUM ('requested', 'granted', 'active', 'expired', 'revoked', 'superseded');
CREATE TYPE user_role AS ENUM ('admin', 'operator', 'maintainer', 'viewer');
CREATE TYPE maintenance_ticket_status AS ENUM ('open', 'in_progress', 'resolved', 'closed');
CREATE TYPE alert_severity AS ENUM ('info', 'warning', 'critical');
