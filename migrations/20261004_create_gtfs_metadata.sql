CREATE TABLE IF NOT EXISTS tracking.gtfs_stops (
    id UUID PRIMARY KEY,
    source TEXT NOT NULL,
    stop_id TEXT NOT NULL,
    stop_name TEXT NOT NULL,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    station_code TEXT,
    country_code CHAR(2),
    operator TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (source, stop_id)
);

CREATE INDEX IF NOT EXISTS idx_gtfs_stops_stop_id
    ON tracking.gtfs_stops (stop_id);

CREATE INDEX IF NOT EXISTS idx_gtfs_stops_name
    ON tracking.gtfs_stops (stop_name);

CREATE TABLE IF NOT EXISTS tracking.gtfs_trips (
    id UUID PRIMARY KEY,
    source TEXT NOT NULL,
    trip_id TEXT NOT NULL,
    route_id TEXT,
    service_name TEXT,
    destination TEXT,
    origin TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (source, trip_id)
);

CREATE INDEX IF NOT EXISTS idx_gtfs_trips_trip_id
    ON tracking.gtfs_trips (trip_id);

CREATE INDEX IF NOT EXISTS idx_gtfs_trips_route_id
    ON tracking.gtfs_trips (route_id);
