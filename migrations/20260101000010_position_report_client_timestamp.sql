-- `reported_at` (existing column) is when the SERVER received a
-- position report — it reflects network arrival order, not the order
-- the train actually occupied those positions. Detecting stale/
-- out-of-order telemetry (Phase 8) requires the train's own clock
-- reading at capture time, so out-of-order network delivery (a real,
-- expected failure mode per the platform's fault-injection design — see
-- crates/shared/mq/src/lib.rs's message_delay_ms_max) can be told apart
-- from a report that is merely late but still the newest one seen.
ALTER TABLE train.position_reports
    ADD COLUMN client_timestamp TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE INDEX idx_position_reports_train_client_time
    ON train.position_reports (train_id, client_timestamp DESC);
