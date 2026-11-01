CREATE TABLE IF NOT EXISTS tracking.train_routes (
    id UUID PRIMARY KEY,
    train_id UUID NOT NULL REFERENCES tracking.trains(id) ON DELETE CASCADE,
    station_id UUID NOT NULL REFERENCES tracking.stations(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    scheduled_arrival TIMESTAMPTZ,
    scheduled_departure TIMESTAMPTZ,
    actual_arrival TIMESTAMPTZ,
    actual_departure TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (train_id, sequence)
);

CREATE INDEX IF NOT EXISTS idx_tracking_train_routes_train
    ON tracking.train_routes (train_id, sequence);

CREATE INDEX IF NOT EXISTS idx_tracking_train_routes_station
    ON tracking.train_routes (station_id);
