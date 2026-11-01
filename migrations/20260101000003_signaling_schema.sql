CREATE SCHEMA IF NOT EXISTS signaling;

CREATE TABLE signaling.stations (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         VARCHAR(128) NOT NULL UNIQUE,
    chainage_m   DOUBLE PRECISION NOT NULL
);

CREATE TABLE signaling.platforms (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    station_id              UUID NOT NULL REFERENCES signaling.stations(id) ON DELETE CASCADE,
    name                    VARCHAR(128) NOT NULL,
    adjacent_block          UUID NOT NULL, -- FK added after blocks table exists
    dwell_time_default_secs INTEGER NOT NULL DEFAULT 30
);

CREATE TABLE signaling.blocks (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name           VARCHAR(128) NOT NULL UNIQUE,
    length_m       DOUBLE PRECISION NOT NULL CHECK (length_m > 0),
    occupancy      occupancy_state NOT NULL DEFAULT 'clear',
    occupied_by    UUID NULL, -- FK added after trains table exists (train.trains)
    last_updated   TIMESTAMPTZ NOT NULL DEFAULT now(),
    max_speed_kph  INTEGER NOT NULL CHECK (max_speed_kph > 0),
    bidirectional  BOOLEAN NOT NULL DEFAULT FALSE
);

ALTER TABLE signaling.platforms
    ADD CONSTRAINT fk_platforms_block FOREIGN KEY (adjacent_block) REFERENCES signaling.blocks(id);

CREATE TABLE signaling.switches (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                  VARCHAR(128) NOT NULL UNIQUE,
    position              switch_position NOT NULL DEFAULT 'normal',
    host_block            UUID NOT NULL REFERENCES signaling.blocks(id),
    normal_route_block    UUID NOT NULL REFERENCES signaling.blocks(id),
    reverse_route_block   UUID NOT NULL REFERENCES signaling.blocks(id),
    last_moved_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked                BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE signaling.signals (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name               VARCHAR(128) NOT NULL UNIQUE,
    kind               signal_kind NOT NULL,
    protects_block     UUID NOT NULL REFERENCES signaling.blocks(id),
    current_aspect     signal_aspect NOT NULL DEFAULT 'red',
    last_aspect_change TIMESTAMPTZ NOT NULL DEFAULT now(),
    chainage_m         DOUBLE PRECISION NOT NULL
);

CREATE TABLE signaling.block_adjacency (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_block               UUID NOT NULL REFERENCES signaling.blocks(id),
    to_block                 UUID NOT NULL REFERENCES signaling.blocks(id),
    via_switch               UUID NULL REFERENCES signaling.switches(id),
    required_switch_position switch_position NULL,
    UNIQUE (from_block, to_block, via_switch)
);

CREATE INDEX idx_blocks_occupancy ON signaling.blocks (occupancy);
CREATE INDEX idx_signals_protects_block ON signaling.signals (protects_block);
CREATE INDEX idx_switches_host_block ON signaling.switches (host_block);
CREATE INDEX idx_adjacency_from_block ON signaling.block_adjacency (from_block);
CREATE INDEX idx_platforms_station ON signaling.platforms (station_id);

-- Convenience view: current network state joined for the dashboard's
-- initial map load, avoiding an N+1 of block/signal/switch queries.
CREATE VIEW signaling.network_state AS
SELECT
    b.id            AS block_id,
    b.name          AS block_name,
    b.occupancy,
    b.occupied_by,
    b.max_speed_kph,
    s.id            AS signal_id,
    s.current_aspect
FROM signaling.blocks b
LEFT JOIN signaling.signals s ON s.protects_block = b.id;
