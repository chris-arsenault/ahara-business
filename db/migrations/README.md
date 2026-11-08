# Migrations

Platform PostgreSQL migration directory for the `ahara-business` database.

`001_create_mail_model.sql` creates the mail schema used by the API and
workers: domains, accepted addresses, contacts, threads, messages, recipients,
attachment refs, forwarding rules, suppressions, and outbound work. The
`002_attachments_retention_forwarding.sql` migration extends attachment,
retention, and forwarding controls. `003_calendar_booking.sql` adds internal
calendar events and booking records. `004_tax_audit_finance.sql` adds
tax/audit expense allocation and manual client receivable tracking.
`005_recurring_expense_occurrences.sql` links recurring expense instances back
to their parent recurring expense. `006_support_mail_routing.sql` activates
`support@ahara.io` for existing domains; its rollback deactivates the address
while preserving existing references. `007_initial_mail_routing.sql` installs
missing initial `ahara.io` domain and address records during normal deployment,
without changing existing operator settings. Its rollback retains routing data
to preserve delivery and references.

`008_s3_attachments.sql` adds owner-bound upload records and version-pinned S3
payload references while retaining existing base64 payloads. Its rollback refuses
to remove S3 references while they own queued or sent mail.

The `rollback/` directory contains the matching rollbacks. The legacy `seed/`
fixture is optional; deployment does not need it to initialize mail routing.
