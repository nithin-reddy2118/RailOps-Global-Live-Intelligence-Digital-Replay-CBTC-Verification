CREATE SCHEMA IF NOT EXISTS train;

CREATE TABLE train.trains (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identifier             VARCHAR(64) NOT NULL UNIQUE,
    operational_state      train_operational_state NOT NULL DEFAULT 'out_of_service',
    current_block          UUID NULL REFERENCES signaling.blocks(id),
    active_authority       UUID NULL, -- FK added after movement_authorities table exists
    max_speed_kph          INTEGER NOT NULL CHECK (max_speed_kph > 0),
    length_m               DOUBLE PRECISION NOT NULL CHECK (length_m > 0),
    last_heartbeat         TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Now that train.trains exists, wire the two forward references left
-- dangling in the signaling schema migration.
ALTER TABLE signaling.blocks
    ADD CONSTRAINT fk_blocks_occupied_by FOREIGN KEY (occupied_by) REFERENCES train.trains(id);

CREATE TABLE train.position_reports (
    id                      BIGSERIAL PRIMARY KEY,
    train_id                UUID NOT NULL REFERENCES train.trains(id) ON DELETE CASCADE,
    occupied_block          UUID NOT NULL REFERENCES signaling.blocks(id),
    offset_within_block_m   DOUBLE PRECISION NOT NULL,
    speed_kph               DOUBLE PRECISION NOT NULL,
    acceleration_mps2       DOUBLE PRECISION NOT NULL,
    reported_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Position reports are high-volume telemetry; index for "latest report
-- per train" queries and time-range queries for the replay/history view.
CREATE INDEX idx_position_reports_train_time ON train.position_reports (train_id, reported_at DESC);

CREATE TABLE train.movement_authorities (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    train_id                 UUID NOT NULL REFERENCES train.trains(id) ON DELETE CASCADE,
    route_id                 UUID NULL,
    block_sequence           UUID[] NOT NULL,
    limit_of_movement_m      DOUBLE PRECISION NOT NULL,
    max_authorized_speed_kph INTEGER NOT NULL CHECK (max_authorized_speed_kph > 0),
    status                   authority_status NOT NULL DEFAULT 'requested',
    issued_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at               TIMESTAMPTZ NOT NULL
);

ALTER TABLE train.trains
    ADD CONSTRAINT fk_trains_active_authority FOREIGN KEY (active_authority) REFERENCES train.movement_authorities(id);

CREATE INDEX idx_authorities_train ON train.movement_authorities (train_id);
CREATE INDEX idx_authorities_status ON train.movement_authorities (status) WHERE status IN ('granted', 'active');

-- A train may have at most one authority in an active/granted state at
-- a time — enforced at the application layer in interlocking-service
-- via row locking (SELECT ... FOR UPDATE on train.trains before issuing),
-- since a partial unique index on (train_id) WHERE status IN (...) would
-- also work but the app-layer lock is required regardless for the
-- multi-step conflict check, so we rely on that rather than duplicating
-- the invariant in two places.
