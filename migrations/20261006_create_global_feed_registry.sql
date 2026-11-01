CREATE TABLE IF NOT EXISTS tracking.global_feed_registry (
    id UUID PRIMARY KEY,
    country_code CHAR(2) NOT NULL,
    operator TEXT NOT NULL,
    feed_name TEXT NOT NULL,
    vehicle_positions_url TEXT,
    trip_updates_url TEXT,
    feed_format TEXT NOT NULL DEFAULT 'GTFS_RT',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    refresh_seconds INTEGER NOT NULL DEFAULT 30,
    last_attempt_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    last_error TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT global_feed_registry_refresh_seconds_check
        CHECK (refresh_seconds >= 5)
);

CREATE UNIQUE INDEX IF NOT EXISTS
    global_feed_registry_country_operator_feed_idx
ON tracking.global_feed_registry (country_code, operator, feed_name);

CREATE INDEX IF NOT EXISTS
    global_feed_registry_enabled_idx
ON tracking.global_feed_registry (enabled);

CREATE INDEX IF NOT EXISTS
    global_feed_registry_country_idx
ON tracking.global_feed_registry (country_code);

INSERT INTO tracking.global_feed_registry (
    id,
    country_code,
    operator,
    feed_name,
    vehicle_positions_url,
    trip_updates_url,
    feed_format,
    enabled,
    refresh_seconds
)
VALUES (
    gen_random_uuid(),
    'NO',
    'Entur',
    'Entur GTFS-RT',
    'https://api.entur.io/realtime/v1/gtfs-rt/vehicle-positions',
    'https://api.entur.io/realtime/v1/gtfs-rt/trip-updates',
    'GTFS_RT',
    TRUE,
    15
)
ON CONFLICT (country_code, operator, feed_name)
DO UPDATE SET
    vehicle_positions_url = EXCLUDED.vehicle_positions_url,
    trip_updates_url = EXCLUDED.trip_updates_url,
    feed_format = EXCLUDED.feed_format,
    enabled = EXCLUDED.enabled,
    refresh_seconds = EXCLUDED.refresh_seconds,
    updated_at = NOW();
