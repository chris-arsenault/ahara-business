# Migrations

Platform PostgreSQL migration directory for the `ahara-business` database.

`001_create_mail_model.sql` creates the mail schema used by the API and
workers: domains, accepted addresses, contacts, threads, messages, recipients,
attachment refs, forwarding rules, suppressions, and outbound work. The
`rollback/` directory contains the matching rollback, and `seed/` contains the
idempotent initial `ahara.io` routing seed.
