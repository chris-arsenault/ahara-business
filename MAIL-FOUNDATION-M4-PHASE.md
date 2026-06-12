# M4 - Backend Foundations

This phase expands only M4 from `MAIL-FOUNDATION-PLAN.md`. The goal is to
turn the existing M0 Lambda scaffolds into a testable backend foundation:
shared config, database pool setup, error handling, authenticated request
context, mail-policy parsing, external-service boundaries, basic API routes,
and worker skeletons that use shared code.

M4 scope guard:
- Do not implement SES inbound MIME parsing, S3 raw MIME fetching, outbound
  sending, bounce/complaint persistence, mailbox list/detail APIs, or any
  frontend mailbox UI. Those belong to M5-M7.
- Do not add MX records, activate the SES receipt rule set, or change AWS
  routability.
- Do not add application-level JWT verification. The shared ALB performs
  `jwt-validation`; the API only extracts already-validated request identity
  from request metadata/headers.

Reference context:
- `MAIL-FOUNDATION-PLAN.md`
- `mail-foundation-spec.md`
- `docs/adr/0001-rust-lambda-and-react-spa.md`
- `docs/adr/0002-public-authenticated-text-only-ui.md`
- `docs/adr/0003-shared-cognito-strong-auth.md`
- `docs/adr/0004-project-owned-mail-infrastructure.md`
- `../ahara/INTEGRATION.md`
- `../ahara-tf-patterns/modules/alb-api/`
- `../ahara-tf-patterns/modules/lambda/`
- Adjacent Rust Lambda patterns in `../tastebase/backend/`

Exit gate:
- `make ci`
- Focused Rust tests for `shared`, `api`, `ingest`, `send-worker`, and
  `feedback-handler` pass before the full gate.

## Step 1 - Add shared configuration loading

Files:
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`
- `backend/shared/src/config.rs`
- `backend/shared/src/lib.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 requires shared config.
- `../ahara/INTEGRATION.md` passes database settings as `DB_HOST`, `DB_PORT`,
  `DB_NAME`, `DB_USERNAME`, and `DB_PASSWORD`, plus project runtime values
  through Lambda environment variables.
- M3 Terraform exports runtime values in `local.common_env`: `MAIL_DOMAIN`,
  `RAW_MAIL_BUCKET`, `RAW_MAIL_PREFIX`, feedback topic ARNs, API URL, app URL,
  and Cognito metadata.

Change:
- Add a shared `AppConfig` with nested database, mail-storage, feedback, API,
  and Cognito config loaded from environment variables.
- Make required values explicit and return a typed error instead of panicking.
- Normalize `RAW_MAIL_PREFIX` so callers can safely join object keys later.
- Keep config parsing pure and unit-testable; do not connect to AWS or
  PostgreSQL in this step.

Verify:
- Before the change:
  `! rg "struct AppConfig|from_env|RAW_MAIL_PREFIX" backend/shared/src backend/shared/Cargo.toml backend/Cargo.toml`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared config`
  Red before: `shared::config::AppConfig` does not exist. Green after: tests
  cover successful env parsing, missing required values, default `DB_PORT`,
  and raw-mail prefix normalization.

## Step 2 - Add shared backend error types  [depends on #1]

Files:
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`
- `backend/shared/src/error.rs`
- `backend/shared/src/lib.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 requires shared error types.
- `mail-foundation-spec.md` requires PII-safe logging and no body/full-header
  leakage.
- ADR-0002 makes the public authenticated API security-sensitive; route errors
  need stable client-facing shapes without leaking internals.

Change:
- Add a shared `AppError` and `AppResult<T>` covering config, auth, validation,
  database, external-service, not-found, forbidden, and internal errors.
- Include a stable public error code/message mapping for API responses, but do
  not require Axum in shared unless the implementation needs it for response
  conversion.
- Ensure secrets and arbitrary internal error details are not emitted as public
  messages.

Verify:
- Before the change:
  `! rg "enum AppError|type AppResult|public_message" backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared error`
  Red before: `shared::error::AppError` does not exist. Green after: tests
  cover config/auth/not-found mappings and verify an internal error's public
  response does not expose the internal detail string.

## Step 3 - Add shared PostgreSQL pool setup  [depends on #1, #2]

Files:
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`
- `backend/shared/src/db.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 requires database pool setup.
- `../ahara/INTEGRATION.md` requires per-project PostgreSQL credentials from
  SSM-backed environment variables, not master credentials.
- Adjacent `../tastebase/backend/shared/src/db.rs` uses `sqlx::PgPool` with
  Rustls for Lambda PostgreSQL access.
- Existing `backend/shared/src/db.rs` already exposes embedded migration SQL
  constants used by M2 tests; keep those exports intact.

Change:
- Add `sqlx` workspace dependencies with PostgreSQL/Rustls support.
- Add a `DbPool` alias and `connect_pool(&AppConfig)` / `database_url(...)`
  helpers using the M4 config values.
- Set a small Lambda-appropriate max connection count.
- Do not run migrations from application code; platform migration tooling owns
  schema application.

Verify:
- Before the change:
  `! rg "PgPool|connect_pool|database_url" backend/shared/src/db.rs backend/Cargo.toml backend/shared/Cargo.toml`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared db`
  Red before: `connect_pool`/`database_url` do not exist. Green after: tests
  cover URL construction from config and confirm migration constants still
  include `CREATE TABLE messages`.

## Step 4 - Add authenticated user context extraction  [depends on #2]

Files:
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`
- `backend/shared/src/auth.rs`
- `backend/shared/src/lib.rs`

Reference behavior:
- ADR-0003 keeps shared Cognito as the identity source.
- `../ahara/INTEGRATION.md` states the frontend sends `Authorization: Bearer
  <access_token>` and the ALB route uses `jwt-validation`; the backend must not
  fall back to app-level JWT validation.
- Adjacent `../tastebase/backend/shared/src/auth.rs` decodes identity claims
  after ALB validation without cryptographic validation.

Change:
- Add a `UserContext` carrying Cognito `sub`, optional email, optional username,
  and optional groups/roles if present in claims.
- Add helpers to extract a bearer token from request headers and decode the
  token payload without verifying signatures.
- Treat malformed or missing tokens as `AppError::Unauthorized`.
- Do not add JWKS fetching, `jsonwebtoken` validation, Cognito network calls, or
  a project-specific identity store.

Verify:
- Before the change:
  `! rg "UserContext|extract_bearer|decode.*claims|RequireAuth" backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared auth`
  Red before: auth symbols do not exist. Green after: tests cover valid bearer
  extraction, lower-case bearer prefix, missing bearer, malformed token, and a
  valid Cognito-style token payload.

## Step 5 - Add mail routing-policy parsing primitives

Files:
- `backend/shared/src/routing.rs`
- `backend/shared/src/lib.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 exit requires policy parsing tests.
- `mail-foundation-spec.md` defines per-domain routing policy values
  `allowlist` and `catchall`, plus `+tag` parsing that matches the base local
  part and retains the full tag for filtering.
- M2 schema enforces `domains.routing_policy IN ('allowlist', 'catchall')` and
  lowercase accepted address local parts.

Change:
- Add `RoutingPolicy` parsing/serialization helpers for `allowlist` and
  `catchall`.
- Add an email-route parser that lowercases domain/base local part, preserves
  a `+tag`, rejects empty local/domain parts, and does not validate deliverable
  email beyond what routing needs.
- Keep this as policy parsing only; do not perform database lookups or final
  allowlist/catchall matching in M4.

Verify:
- Before the change:
  `! rg "RoutingPolicy|plus_tag|parse_route" backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared routing`
  Red before: routing symbols do not exist. Green after: tests cover both
  policy strings, invalid policy strings, case normalization, plus-tag
  extraction, and invalid route addresses.

## Step 6 - Add external-service trait boundaries and in-memory doubles

Files:
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`
- `backend/shared/src/ports.rs`
- `backend/shared/src/lib.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 requires external-service traits for SES/S3/SNS
  and in-memory service doubles.
- ADR-0001 requires shared logic to remain testable outside Lambda handlers.
- ADR-0004 keeps SES/S3/SNS resources project-owned, but M4 should only define
  boundaries; concrete AWS SDK behavior belongs to the phase that first needs
  it.

Change:
- Add traits for raw-mail storage, outbound mail sending, and feedback/event
  publishing using request/response structs that do not expose AWS SDK types.
- Add in-memory test doubles for those traits under `#[cfg(test)]` or a test
  helper module usable by API route tests.
- Do not implement real AWS SDK clients in M4.

Verify:
- Before the change:
  `! rg "RawMailStore|MailSender|FeedbackPublisher|InMemory" backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared ports`
  Red before: service traits/doubles do not exist. Green after: tests verify
  the in-memory doubles record calls and can simulate success and failure.

## Step 7 - Refactor the API crate into a testable library  [depends on #1, #2, #3]

Files:
- `backend/api/Cargo.toml`
- `backend/api/src/lib.rs`
- `backend/api/src/main.rs`

Reference behavior:
- ADR-0001 says shared logic belongs in library crates so behavior is testable
  outside Lambda glue.
- `../ahara/INTEGRATION.md` routes `/health` unauthenticated and all other API
  paths authenticated through the shared ALB.
- Current `backend/api/src/main.rs` has a minimal `/health` router that should
  keep returning `{ "status": "ok", "service": "ahara-business" }`.

Change:
- Move router construction and handler code into `api/src/lib.rs`.
- Keep `api/src/main.rs` as Lambda startup glue: initialize tracing, load
  config, create DB pool, build real application state, and run
  `lambda_http`.
- Add route-test utilities using in-memory state; keep the `/health` route
  unauthenticated and independent from DB/AWS.

Verify:
- Before the change:
  `test ! -f backend/api/src/lib.rs`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p api health`
  Red before: the `api` library target/testable router does not exist. Green
  after: a route test calls `GET /health` and asserts status `200`, service
  name, and no auth requirement.

## Step 8 - Add the authenticated user context API route  [depends on #4, #7]

Files:
- `backend/api/src/lib.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 requires authenticated user context.
- ADR-0002 and ADR-0003 make every non-health application route authenticated
  through shared Cognito and the shared ALB.
- The API must expose only request-user metadata; it must not create project
  users, contacts, or trusted sender identity from display names in this step.

Change:
- Add `GET /me` returning the extracted `UserContext`.
- Require auth extraction for `/me`; missing or malformed bearer metadata
  returns unauthorized.
- Do not add session mutation, cookies, CSRF behavior, or user provisioning in
  M4.

Verify:
- Before the change:
  `! rg "route\\(\"/me\"|UserContext" backend/api/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p api me`
  Red before: `/me` does not exist. Green after: tests cover valid user context
  response and unauthorized response when auth metadata is absent.

## Step 9 - Add domain/address config service and API routes  [depends on #5, #7, #8]

Files:
- `backend/shared/src/domain_config.rs`
- `backend/shared/src/lib.rs`
- `backend/api/src/lib.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 requires domain/address config routes.
- `mail-foundation-spec.md` defines configured domains, routing policies, and
  allowlist address entries.
- M2 seed data established `ahara.io` with allowlist entries for `chris` and
  `contact`; M4 should expose config management foundations without making
  inbound mail routable.

Change:
- Add shared request/response types and a service trait for:
  - listing configured domains with accepted addresses
  - updating a domain routing policy/active flag
  - creating or reactivating an accepted address local part
  - deactivating an accepted address
- Add an in-memory implementation for tests and a PostgreSQL-backed
  implementation using the M2 tables.
- Add authenticated API routes:
  - `GET /domains`
  - `PATCH /domains/{domain_name}`
  - `POST /domains/{domain_name}/addresses`
  - `DELETE /domains/{domain_name}/addresses/{local_part}`
- Reuse the routing-policy and address parser from Step 5.
- Do not create DNS, SES receipt rules, or MX records from these routes.

Verify:
- Before the change:
  `! rg "domain_config|/domains|AcceptedAddress" backend/shared/src backend/api/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p api domains`
  and
  `cargo test --manifest-path backend/Cargo.toml -p shared domain_config`
  Red before: service/types/routes do not exist. Green after: in-memory route
  tests cover list, policy update, address add/reactivate, address deactivate,
  auth required, and invalid routing policy rejection.

## Step 10 - Add contact basics service and API routes  [depends on #7, #8]

Files:
- `backend/shared/src/contacts.rs`
- `backend/shared/src/lib.rs`
- `backend/api/src/lib.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 requires contact basics.
- `mail-foundation-spec.md` defines a thin `contact` table as a CRM seam with
  name, primary address, notes, and message associations later.
- Security requirements say `From` is display-only and unauthenticated senders
  must not be auto-associated to contacts without explicit verification.

Change:
- Add shared request/response types and a service trait for contact basics:
  list contacts, create contact, update contact, and fetch one contact.
- Add an in-memory implementation for tests and a PostgreSQL-backed
  implementation using the M2 `contacts` table.
- Add authenticated API routes:
  - `GET /contacts`
  - `POST /contacts`
  - `GET /contacts/{contact_id}`
  - `PATCH /contacts/{contact_id}`
- Normalize primary email address when provided; reject invalid primary-address
  pairs that would violate the M2 schema.
- Do not add message-contact association behavior in M4.

Verify:
- Before the change:
  `! rg "contacts|/contacts|Contact" backend/shared/src backend/api/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p api contacts`
  and
  `cargo test --manifest-path backend/Cargo.toml -p shared contacts`
  Red before: contact service/types/routes do not exist. Green after:
  in-memory route tests cover list/create/get/update, auth required, normalized
  primary address, and not-found behavior.

## Step 11 - Put worker Lambdas behind shared handler libraries  [depends on #1, #2]

Files:
- `backend/ingest/Cargo.toml`
- `backend/ingest/src/lib.rs`
- `backend/ingest/src/main.rs`
- `backend/send-worker/Cargo.toml`
- `backend/send-worker/src/lib.rs`
- `backend/send-worker/src/main.rs`
- `backend/feedback-handler/Cargo.toml`
- `backend/feedback-handler/src/lib.rs`
- `backend/feedback-handler/src/main.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 builds worker skeletons around shared logic.
- ADR-0001 requires Lambda glue to stay thin and behavior testable outside the
  runtime entrypoint.
- M5-M7 own actual ingest, send, and feedback semantics; M4 must not implement
  those behaviors.

Change:
- Extract each existing no-op worker handler into a library function that:
  loads or accepts `AppConfig`, logs only non-PII request metadata, and returns
  a stable `{ "status": "ok" }`-style response.
- Keep each `main.rs` as runtime glue that initializes tracing and calls the
  library handler through `lambda_runtime`.
- Do not parse SES events, fetch raw MIME, send mail, or persist feedback.

Verify:
- Before the change:
  `test ! -f backend/ingest/src/lib.rs && test ! -f backend/send-worker/src/lib.rs && test ! -f backend/feedback-handler/src/lib.rs`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p ingest -p send-worker -p feedback-handler`
  Red before: worker library targets/handler symbols do not exist. Green after:
  tests call each library handler with a dummy JSON event and assert the stable
  no-op response.

## Step 12 - Run M4 exit gate  [depends on #1, #2, #3, #4, #5, #6, #7, #8, #9, #10, #11]

Files:
- None

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M4 exit requires `make ci` green and unit tests
  covering config, auth extraction, policy parsing, and in-memory service
  doubles.
- The M4 scope guard requires no M5-M7 behavior and no routability changes.

Change:
- No code changes in this step.

Verify:
- Run focused checks:
  `cargo test --manifest-path backend/Cargo.toml -p shared`
  `cargo test --manifest-path backend/Cargo.toml -p api`
  `cargo test --manifest-path backend/Cargo.toml -p ingest -p send-worker -p feedback-handler`
- Run the full gate:
  `make ci`
- Source scan after the gate:
  `! rg "aws_ses_active_receipt_rule_set|type\\s*=\\s*\"MX\"" infrastructure/terraform`
  and
  `! rg "parse.*MIME|raw MIME|SendRawEmail|SendEmail|ses_message_id" backend/ingest backend/send-worker backend/feedback-handler backend/api/src`
