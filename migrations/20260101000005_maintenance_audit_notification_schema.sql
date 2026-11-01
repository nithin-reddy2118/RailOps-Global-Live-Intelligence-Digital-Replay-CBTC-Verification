CREATE SCHEMA IF NOT EXISTS maintenance;
CREATE SCHEMA IF NOT EXISTS audit;
CREATE SCHEMA IF NOT EXISTS notification;

CREATE TABLE maintenance.tickets (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_description VARCHAR(256) NOT NULL,
    related_block     UUID NULL REFERENCES signaling.blocks(id),
    related_switch    UUID NULL REFERENCES signaling.switches(id),
    related_signal    UUID NULL REFERENCES signaling.signals(id),
    status            maintenance_ticket_status NOT NULL DEFAULT 'open',
    opened_by         UUID NOT NULL REFERENCES auth.users(id),
    opened_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at       TIMESTAMPTZ NULL,
    notes             TEXT NULL
);

CREATE INDEX idx_maintenance_status ON maintenance.tickets (status);

-- Append-only audit log: every safety-relevant and administrative
-- action across every service writes here via audit-log-service. No
-- UPDATE or DELETE grants are issued on this table in production
-- (enforced via a dedicated DB role, provisioned outside migrations).
CREATE TABLE audit.log_entries (
    id             BIGSERIAL PRIMARY KEY,
    event_id       UUID NOT NULL,
    causality_id   UUID NOT NULL,
    origin_service VARCHAR(64) NOT NULL,
    actor_user_id  UUID NULL REFERENCES auth.users(id),
    action         VARCHAR(128) NOT NULL,
    details        JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_occurred_at ON audit.log_entries (occurred_at DESC);
CREATE INDEX idx_audit_causality ON audit.log_entries (causality_id);
CREATE INDEX idx_audit_actor ON audit.log_entries (actor_user_id);

CREATE TABLE notification.alerts (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    severity    alert_severity NOT NULL,
    title       VARCHAR(256) NOT NULL,
    description TEXT NOT NULL,
    source_service VARCHAR(64) NOT NULL,
    acknowledged   BOOLEAN NOT NULL DEFAULT FALSE,
    acknowledged_by UUID NULL REFERENCES auth.users(id),
    acknowledged_at TIMESTAMPTZ NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_alerts_unacked ON notification.alerts (acknowledged, severity) WHERE NOT acknowledged;
