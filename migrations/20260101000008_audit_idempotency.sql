-- Backs audit-log-service's `ON CONFLICT DO NOTHING` insert (see
-- services/audit-log-service/src/repository.rs) with an actual
-- constraint, so a message redelivered after a retry (see the
-- ack/nack policy in crates/shared/mq/src/lib.rs::consume) or
-- duplicated by fault-injection (crates/shared/mq/src/lib.rs::publish)
-- is deduplicated on `event_id` rather than silently creating a second
-- audit row for the same event.
ALTER TABLE audit.log_entries
    ADD CONSTRAINT uq_audit_log_entries_event_id UNIQUE (event_id);
