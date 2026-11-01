CREATE SCHEMA IF NOT EXISTS ptdf;

CREATE TABLE ptdf.runs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    label       VARCHAR(256) NOT NULL,
    started_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at    TIMESTAMPTZ NULL,
    status      VARCHAR(32) NOT NULL DEFAULT 'recording' -- recording | completed | replaying | replayed
);

-- `id` is BIGSERIAL rather than a per-run counter: insertion order (the
-- order the database actually assigned monotonically increasing ids in)
-- IS the recorded causal order for replay purposes. This is simpler and
-- more robust than maintaining a separate per-run sequence counter
-- under concurrent writers, and is exactly the property replay depends
-- on: entries for a run replay in the same total order they were
-- recorded in, regardless of which service produced which event.
CREATE TABLE ptdf.trace_entries (
    id             BIGSERIAL PRIMARY KEY,
    run_id         UUID NOT NULL REFERENCES ptdf.runs(id) ON DELETE CASCADE,
    event_id       UUID NOT NULL,
    causality_id   UUID NOT NULL,
    origin_service VARCHAR(64) NOT NULL,
    event_type     VARCHAR(128) NOT NULL,
    payload        JSONB NOT NULL,
    recorded_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_ptdf_trace_run_order ON ptdf.trace_entries (run_id, id);
CREATE INDEX idx_ptdf_trace_causality ON ptdf.trace_entries (causality_id);
