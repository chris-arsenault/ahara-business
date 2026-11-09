# Changelog

All notable user-visible changes are recorded here.

## Unreleased

- Fix direct attachment uploads rejected by S3 CORS when Lambda's SDK includes
  `x-amzn-trace-id` in the upload headers.
- Overhaul mailbox navigation, conversation lists, the reading pane, compose and
  inline replies; use space efficiently on desktop and phones, with expandable
  security details.
- Show outbound replies and their delivery status within conversations.
- Upload and download private attachments directly through S3, bypassing the
  application WAF. Allow 15 MiB combined, show upload/removal states, and send
  through SES v2. Preserve existing attachments and failed-send drafts.
- Initialize missing mail domain and address routing during normal migrations,
  preserving existing operator settings.
- Update `async-trait` to fix generated-code Clippy errors blocking deployment.
- Added `support@ahara.io` to SES receipt routing, receipt-gate recipients, and
  application address routing, including a migration for existing deployments.

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
