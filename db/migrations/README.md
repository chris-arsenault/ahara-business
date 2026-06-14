# Migrations

Platform PostgreSQL migration directory for the `ahara-business` database.

`001_create_mail_model.sql` creates the mail schema used by the API and
workers: domains, accepted addresses, contacts, threads, messages, recipients,
attachment refs, forwarding rules, suppressions, and outbound work. The
`002_attachments_retention_forwarding.sql` migration extends attachment,
retention, and forwarding controls. `003_calendar_booking.sql` adds internal
calendar events and booking records. The `rollback/` directory contains the
matching rollbacks, and `seed/` contains the idempotent initial `ahara.io`
routing seed.
