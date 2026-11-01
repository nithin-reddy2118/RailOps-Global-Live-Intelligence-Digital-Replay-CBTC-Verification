-- Lets notification-service dedupe alerts created from a redelivered or
-- fault-injection-duplicated event, the same idempotency concern as the
-- audit log (see 20260101000008_audit_idempotency.sql). Nullable+partial
-- unique index rather than NOT NULL UNIQUE because manually-created
-- alerts (none exist yet, but the schema shouldn't foreclose them) have
-- no source event to key on.
ALTER TABLE notification.alerts
    ADD COLUMN source_event_id UUID NULL;

CREATE UNIQUE INDEX uq_alerts_source_event_id
    ON notification.alerts (source_event_id)
    WHERE source_event_id IS NOT NULL;
