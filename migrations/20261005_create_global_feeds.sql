CREATE TABLE IF NOT EXISTS tracking.global_feeds (
    id UUID PRIMARY KEY,
    provider TEXT NOT NULL,
    name TEXT,
    country_code CHAR(2),
    data_type TEXT NOT NULL,
    realtime_vehicle_positions_url TEXT,
    realtime_trip_updates_url TEXT,
    static_gtfs_url TEXT,
    source_url TEXT,
    is_official BOOLEAN,
    status TEXT,
    last_checked_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (source_url)
);

CREATE INDEX IF NOT EXISTS idx_global_feeds_country
    ON tracking.global_feeds(country_code);

CREATE INDEX IF NOT EXISTS idx_global_feeds_type
    ON tracking.global_feeds(data_type);

CREATE INDEX IF NOT EXISTS idx_global_feeds_status
    ON tracking.global_feeds(status);
