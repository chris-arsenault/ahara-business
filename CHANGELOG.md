# Changelog

All notable user-visible changes are recorded here.

## v0.1.0 - 2026-06-12

### Mail

- Shipped Ahara Mail as an authenticated SES-backed mailbox for `ahara.io`.
- Added inbound receipt gating, raw MIME storage, MIME parsing, plaintext
  mailbox persistence, spam/virus disposition, forwarding enqueue, and
  flood-control enforcement.
- Added compose, reply, sent mail, SES send worker retries, bounce/complaint
  feedback, and recipient suppression.

### Web UI

- Added Cognito sign-in, TOTP setup, mailbox reads, thread detail, search,
  read/unread state, contacts, routing admin, forwarding admin, text-only
  compose/reply, and sent mail views.

### Operations

- Added project-owned SES, S3, SNS, Lambda, Route 53, CloudWatch alarm, and
  CloudFront-backed frontend infrastructure.

### Bug fixes

- Fixed SES raw-mail S3 key handling so async ingest validates the configured
  receipt-rule S3 action location.

### Documentation

- Updated the documentation surface to current-state docs, ADRs, operations,
  deploy, smoke-check, and backlog references.
