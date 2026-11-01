CREATE SCHEMA IF NOT EXISTS tracking;

CREATE TABLE IF NOT EXISTS tracking.trains (
    id UUID PRIMARY KEY,
    train_number TEXT NOT NULL,
    operator TEXT,
    country_code CHAR(2),
    service_name TEXT,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    speed_kmh DOUBLE PRECISION,
    bearing DOUBLE PRECISION,
    current_station TEXT,
    next_station TEXT,
    destination TEXT,
    position_timestamp TIMESTAMPTZ,
    source TEXT NOT NULL,
    data_status TEXT NOT NULL DEFAULT 'UNAVAILABLE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tracking_trains_number
    ON tracking.trains (train_number);

CREATE INDEX IF NOT EXISTS idx_tracking_trains_country
    ON tracking.trains (country_code);

CREATE INDEX IF NOT EXISTS idx_tracking_trains_position
    ON tracking.trains (latitude, longitude);

CREATE TABLE IF NOT EXISTS tracking.stations (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    country_code CHAR(2),
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    station_code TEXT,
    operator TEXT,
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tracking_stations_name
    ON tracking.stations (name);

CREATE INDEX IF NOT EXISTS idx_tracking_stations_code
    ON tracking.stations (station_code);

CREATE INDEX IF NOT EXISTS idx_tracking_stations_position
    ON tracking.stations (latitude, longitude);
