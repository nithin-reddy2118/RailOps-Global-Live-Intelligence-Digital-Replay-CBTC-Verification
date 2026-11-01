-- Live train passenger/occupancy support.
ALTER TABLE tracking.trains
    ADD COLUMN IF NOT EXISTS passenger_count INTEGER,
    ADD COLUMN IF NOT EXISTS passenger_capacity INTEGER,
    ADD COLUMN IF NOT EXISTS occupancy_percentage DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS occupancy_status TEXT,
    ADD COLUMN IF NOT EXISTS passenger_count_source TEXT;

CREATE TABLE IF NOT EXISTS tracking.train_occupancy_events (
    id UUID PRIMARY KEY,
    train_id UUID NOT NULL REFERENCES tracking.trains(id) ON DELETE CASCADE,
    trip_id TEXT,
    station_id UUID REFERENCES tracking.stations(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL CHECK (event_type IN ('BOARD', 'ALIGHT')),
    passenger_count INTEGER NOT NULL CHECK (passenger_count >= 0),
    source TEXT NOT NULL,
    event_timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_train_occupancy_events_train_time
    ON tracking.train_occupancy_events(train_id, event_timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_train_occupancy_events_trip_time
    ON tracking.train_occupancy_events(trip_id, event_timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_trains_live_occupancy
    ON tracking.trains(data_status, updated_at DESC);
