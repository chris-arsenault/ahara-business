# M2 - Database Model And Migrations

This phase expands only M2 from `MAIL-FOUNDATION-PLAN.md`. The plan remains the
single source of truth for scope: build the mail storage model, rollback, seed
data, and PostgreSQL-backed verification. Do not start ingestion, SES, UI, or
mailbox behavior in this phase.

Reference context:
- `mail-foundation-spec.md`
- `docs/adr/0001-rust-lambda-react.md`
- `docs/adr/0002-public-authenticated-text-only-mail-ui.md`
- `docs/adr/0003-shared-cognito-auth.md`
- `docs/adr/0004-project-owned-mail-infrastructure.md`
- `../ahara/INTEGRATION.md`
- Adjacent migration examples in `../tastebase/db/migrations` and `../dosekit/db/migrations`
- Docker CLI wrapper behavior in this environment: use `docker run`, `docker ps`,
  and `docker rm -f`; do not rely on a local Docker API socket.

Exit gate:
- `make ci`
- Storage integration tests apply the forward migration, apply the seed twice,
  apply rollback, and re-apply the migration against real PostgreSQL in a
  temporary Docker-started Postgres container.

## Step 1 - [DECISION] Confirm recipient storage shape

Files:
- None

Decision:
- Accepted on 2026-06-10: use normalized `recipients` rows.

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M2 names this as an explicit decision.
- `mail-foundation-spec.md` leaves recipient storage open as either normalized
  rows or arrays on `message`.
- The UI/search model needs per-recipient addressing for To/Cc/Bcc display and
  address-based filtering.

Change:
- Implement normalized `recipients` rows.

Verify:
- No command. This is a required semantic decision before schema work.

## Step 2 - [DECISION] Confirm outbound work storage shape

Files:
- None

Decision:
- Accepted on 2026-06-10: use a dedicated `outbound_work` table.

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M2 names this as an explicit decision.
- `mail-foundation-spec.md` leaves outbound queue shape open as either a
  dedicated work table or status fields folded entirely into `message`.
- Sending behavior later needs retry scheduling, lock ownership, idempotency,
  suppression checks, and failure details without overloading mailbox message
  state.

Change:
- Implement a dedicated `outbound_work` table.

Verify:
- No command. This is a required semantic decision before schema work.

## Step 3 - [DECISION] Confirm initial routing seed values

Files:
- None

Decision:
- Resolved on 2026-06-10:
  - Do not seed the temporary frontend hostname `ahara-business.ahara.io`.
  - Use the future product/mail domain `ahara.io` for application-side routing
    rows.
  - Seed `chris` and `contact` as accepted local parts.
  - Keep the system unroutable: do not add MX DNS records, SES receipt
    routing, or any other inbound mail delivery route in M2.

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M2 requires idempotent seed data for the initial
  domain/address routing.
- `mail-foundation-spec.md` defines accepted domain and address records but does
  not name the initial mail domain or accepted local parts.
- `../ahara/INTEGRATION.md` requires seed files to be idempotent with
  `ON CONFLICT`.
- The product domain is being reconsidered. The likely target is bare
  `ahara.io`, with the current Ahara portal moving to `cv.ahara.io`; that
  portal move is shared/domain infrastructure work outside M2.
- M2 is database-only. It may create application-side routing rows, but it must
  not create MX records or otherwise make inbound mail routable.

Change:
- Use the confirmed initial seed values: domain `ahara.io`, local parts
  `chris` and `contact`.

Verify:
- No command. This decides seed data, not behavior.

## Step 4 - Add forward mail model migration

Files:
- `db/migrations/001_create_mail_model.sql`

Reference behavior:
- `mail-foundation-spec.md` Storage/Data Model defines domains, accepted
  addresses, messages, recipients, attachment refs, threads, contacts,
  forwarding rules, suppressions, and outbound work.
- `../ahara/INTEGRATION.md` requires project migrations under
  `db/migrations`, lexicographic zero-padded names, schema/data only, and no
  role/database/grant DDL.
- Adjacent project migrations use `gen_random_uuid()` UUID primary keys and
  plain PostgreSQL DDL.

Change:
- Add a single forward migration that creates:
  - `domains`
  - `addresses`
  - `contacts`
  - `threads`
  - `messages`
  - `recipients` after Step 1 confirms normalized rows
  - `attachment_refs`
  - `forwarding_rules`
  - `suppressions`
  - `outbound_work` after Step 2 confirms a dedicated work table
- Include check constraints for enumerated storage values:
  - domain routing policy: `allowlist`, `catchall`
  - message direction: `inbound`, `outbound`
  - auth results: `pass`, `fail`, `neutral`, `softfail`, `temperror`,
    `permerror`, `none`
  - message status: `received`, `queued`, `sending`, `sent`, `failed`,
    `bounced`, `complained`
  - recipient kind: `to`, `cc`, `bcc`
  - forwarding rule kind: `domain`, `address`
  - suppression reason: `bounce`, `complaint`, `manual`
  - outbound work status: `queued`, `sending`, `sent`, `failed`, `bounced`,
    `complained`
- Include indexes required by the spec-level access patterns:
  - mailbox list by direction and received date
  - unread inbound list
  - thread message list
  - contact message list
  - message lookup by SES message id and S3 raw key
  - recipient lookup by address
  - outbound work pickup by status and next attempt
  - suppression lookup by address
- Do not create roles, users, grants, default privileges, or databases.

Verify:
- Before the change, confirm the migration does not exist:
  `test ! -f db/migrations/001_create_mail_model.sql`
- After the change:
  `rg "CREATE TABLE (domains|addresses|messages|recipients|outbound_work)" db/migrations/001_create_mail_model.sql`
  and
  `! rg "CREATE (ROLE|USER|DATABASE)|GRANT|REVOKE|ALTER DEFAULT PRIVILEGES" db/migrations/001_create_mail_model.sql`

## Step 5 - Add rollback migration

Files:
- `db/migrations/rollback/001_create_mail_model.sql`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M2 requires rollback files.
- `../ahara/INTEGRATION.md` expects rollback files under
  `db/migrations/rollback` with filenames matching the forward migration.

Change:
- Add rollback SQL that drops the M2 tables in dependency-safe reverse order.
- Keep rollback scoped to objects created by
  `db/migrations/001_create_mail_model.sql`.

Verify:
- Before the change, confirm the rollback does not exist:
  `test ! -f db/migrations/rollback/001_create_mail_model.sql`
- After the change:
  `rg "DROP TABLE IF EXISTS" db/migrations/rollback/001_create_mail_model.sql`

## Step 6 - Add idempotent initial routing seed

Files:
- `db/migrations/seed/001_initial_routing.sql`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M2 requires idempotent seed data for initial
  domain/address routing.
- `../ahara/INTEGRATION.md` requires seed SQL to be idempotent with
  `ON CONFLICT`.
- Step 3 provides the exact seed domain and local parts.

Change:
- Add a seed file that upserts the initial `domains` row and accepted
  `addresses` rows.
- Use `ON CONFLICT` so the seed can run repeatedly without duplicate rows.
- Preserve active routes on repeat execution.
- Do not add or imply any MX records, Route53 DNS changes, SES receipt rules,
  or inbound delivery wiring. This seed is application-side state only.

Verify:
- Before the change, confirm the seed does not exist:
  `test ! -f db/migrations/seed/001_initial_routing.sql`
- After the change:
  `rg "ON CONFLICT" db/migrations/seed/001_initial_routing.sql`

## Step 7 - Add PostgreSQL storage test support

Files:
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`
- `backend/shared/src/lib.rs`
- `backend/shared/src/db.rs`

Reference behavior:
- ADR-0001 keeps reusable backend behavior in Rust library crates.
- `../ahara-infra/backend/db-migrate/tests/integration.rs` is the reference for
  real PostgreSQL verification.
- The M2 plan requires Rust storage tests with real PostgreSQL.
- This environment exposes Docker through a CLI wrapper, not a local Docker API
  socket, so the storage test must use the Docker CLI wrapper directly.

Change:
- No additional Rust database client dependency is needed; the integration test
  drives `psql` through the supported Docker CLI wrapper.
- Add a small shared DB test helper module that exposes migration, rollback,
  and seed SQL through `include_str!`.
- Keep the helper test-oriented; do not introduce production repositories or
  data-access abstractions in M2.

Verify:
- Before the change, confirm the helper module is absent:
  `test ! -f backend/shared/src/db.rs`
- After the change:
  `cargo fmt --manifest-path backend/Cargo.toml --check`
  and
  `cargo check --manifest-path backend/Cargo.toml -p shared --tests`

## Step 8 - Add mail model integration tests

Files:
- `backend/shared/tests/mail_model.rs`

Reference behavior:
- M2 requires real PostgreSQL verification.
- This environment's Docker access is the supported Docker CLI wrapper, not a
  local Docker API socket.
- The migration and seed behavior must prove:
  - forward migration applies
  - seed can run twice
  - rollback removes M2 tables
  - migration can be re-applied after rollback

Change:
- Add a `mail_model` integration test that starts a temporary Postgres Docker
  container through the local Docker CLI wrapper,
  applies `001_create_mail_model.sql`, applies
  `seed/001_initial_routing.sql` twice, asserts the expected seed row counts,
  applies `rollback/001_create_mail_model.sql`, verifies M2 tables are gone,
  and re-applies the forward migration.

Verify:
- Before the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared --test mail_model`
  must fail because the test target does not exist yet.
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared --test mail_model -- --nocapture`

## Step 9 - Wire storage integration tests into CI

Files:
- `Makefile`

Reference behavior:
- M2 exit gate is `make ci` green plus local migration/rollback/seed
  verification.
- The existing CI target is the project-local verification surface.

Change:
- Add the `shared` mail model integration test command to the existing `test`
  target so `make ci` runs the storage verification required by M2.
- Keep the existing frontend, formatting, linting, backend unit test, and
  coverage commands intact.

Verify:
- Before the change:
  `! rg "mail_model" Makefile`
- After the change:
  `rg "cargo test .*mail_model" Makefile`

## Step 10 - Run M2 exit gate

Files:
- None

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M2 exit requires `make ci` green, migrations apply
  and roll back locally, and idempotent seeds can run twice.

Change:
- No code changes in this step.

Verify:
- Run `make ci`.
- In the phase report, include the actual relevant output showing:
  - `cargo test --manifest-path backend/Cargo.toml -p shared --test mail_model`
    passed
  - seed ran twice in the integration test without duplicates
  - rollback and re-apply passed
  - the overall `make ci` command exited successfully
