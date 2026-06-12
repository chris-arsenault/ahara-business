# M5 - Inbound Ingest Pipeline

This phase expands only M5 from `MAIL-FOUNDATION-PLAN.md`. The goal is to
turn the M4 ingest scaffold into a real SES inbound pipeline: parse the SES
receipt event, fetch the raw MIME object from private S3, derive safe text and
metadata, enforce routing and security disposition, and persist an idempotent
inbound audit/mail record in PostgreSQL.

M5 scope guard:
- Do not add mailbox read APIs, frontend mailbox UI, compose/send behavior,
  forwarding behavior, feedback persistence, or bounce/complaint handling.
  Those belong to M6-M7.
- Do not add MX records, activate the SES receipt rule set, or change AWS
  routability.
- Do not extract attachment bytes into application storage. Persist attachment
  metadata only; raw bytes remain in the raw MIME S3 object.
- Do not store sender HTML for rendering. Store only the selected plaintext
  body produced by `text/plain` extraction or HTML-to-text conversion.
- Do not generate app-side bounces for unknown, suspicious, over-limit, spam,
  indeterminate, or virus-failed inbound mail.
- Do not log message bodies, raw MIME bytes, or full headers.

Reference context:
- `MAIL-FOUNDATION-PLAN.md`
- `mail-foundation-spec.md`
- `MAIL-FOUNDATION-M4-PHASE.md`
- `docs/adr/0001-rust-lambda-and-react-spa.md`
- `docs/adr/0002-public-authenticated-text-only-ui.md`
- `docs/adr/0004-project-owned-mail-infrastructure.md`
- `../ahara/INTEGRATION.md`
- Current M4 shared modules:
  `backend/shared/src/config.rs`, `backend/shared/src/db.rs`,
  `backend/shared/src/error.rs`, `backend/shared/src/routing.rs`,
  `backend/shared/src/mail_security.rs`, `backend/shared/src/domain_config.rs`,
  `backend/shared/src/contacts.rs`, and `backend/shared/src/ports.rs`
- M2 storage model:
  `db/migrations/001_create_mail_model.sql`
- M3 Terraform shape:
  `infrastructure/terraform/mail_receiving.tf`

Exit gate:
- Focused Rust tests for shared inbound parsing, policy, persistence, and the
  ingest handler pass.
- `make ci`
- Source scan confirms this phase did not add MX records, activate receipt
  rules, or add outbound/frontend mailbox behavior.

## Step 1 - Confirm ingest cap values  [DECISION]

Files:
- No code files. This is the only M5 semantics decision before implementation.

Reference behavior:
- `mail-foundation-spec.md` requires hard reject caps for max message size,
  max multipart nesting depth, and max attachment count.
- `MAIL-FOUNDATION-PLAN.md` M5 requires parser tests for ingest caps.
- The plan/spec do not name numeric values, and those values affect data loss,
  cost, Lambda memory pressure, and legitimate large-message handling.

Change:
- Stop for user confirmation before editing code.
- Recommended defaults unless the user chooses different values:
  max raw MIME size `25 MiB`, max MIME nesting depth `20`, max attachment refs
  `25`.
- The confirmed values become the `IngestLimits::default()` values in step 4.
- M5 does not add a separate per-attachment byte limit because attachment bytes
  are not extracted; the raw message size cap is the byte cap.

Verify:
- No automated verification. The executor records the confirmed cap values and
  then proceeds to step 2.

## Step 2 - Add typed SES receipt event parsing

Files:
- `backend/shared/src/inbound/mod.rs`
- `backend/shared/src/inbound/ses_event.rs`
- `backend/shared/src/lib.rs`
- `backend/shared/tests/fixtures/inbound/ses_receipt_clean.json`
- `backend/shared/tests/fixtures/inbound/ses_receipt_multi_record.json`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M5 requires parsing SES event metadata and
  idempotency by SES message ID/S3 key.
- `mail-foundation-spec.md` says the receipt rule stores raw MIME to S3 and
  triggers parse; ingest extracts SES-provided SPF/DKIM/DMARC/spam/virus
  verdicts and recipients.
- M3 Terraform wires an SES S3 receipt action before the ingest Lambda action.
- ADR-0001 requires parsing behavior to live in testable shared library code,
  not only Lambda glue.

Change:
- Add an `inbound` module and typed serde structs for the SES receipt Lambda
  payload.
- Add a parser that returns one `InboundReceipt` per event record with:
  SES message ID, receipt timestamp if present, SES envelope recipients,
  SPF/DKIM/DMARC verdict strings, spam/virus verdict strings, S3 bucket, and
  S3 object key.
- Validate that required values are present: record source, SES message ID,
  at least one recipient, S3 bucket, and S3 object key.
- Keep unknown SES fields ignored so AWS event additions do not break ingest.
- Do not fetch S3 or parse MIME in this step.

Verify:
- Before the change:
  `! rg "InboundReceipt|SesReceiptEvent|RawMailPointer" backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared ses_event`
- Red before: the `shared::inbound::ses_event` symbols and fixtures do not
  exist. Green after: tests cover a clean one-record event, a multi-record
  event, S3 bucket/key extraction, SES verdict extraction, and required-field
  validation.

## Step 3 - Add the real S3 raw-mail store

Files:
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`
- `backend/shared/src/raw_mail_store.rs`
- `backend/shared/src/lib.rs`
- `backend/shared/src/ports.rs`

Reference behavior:
- ADR-0004 says this project owns private raw MIME S3 storage.
- `mail-foundation-spec.md` requires raw MIME to remain private S3 data.
- M4 already defined the `RawMailStore` trait and in-memory test double; M5 is
  the first phase that needs a concrete AWS implementation.
- M4 config normalizes `RAW_MAIL_PREFIX`; M3 Terraform configures the SES S3
  object prefix.

Change:
- Add AWS SDK dependencies needed for S3 only.
- Add `S3RawMailStore` implementing `RawMailStore`.
- Fetch objects from the configured raw-mail bucket using the SES event object
  key; require the key to stay under the configured raw-mail prefix.
- Return raw bytes without logging object contents.
- Implement `put_raw_mail` only as the trait requires; keep M5 production flow
  read-only from SES-created S3 objects.
- Keep AWS SDK types out of the trait API.

Verify:
- Before the change:
  `! rg "S3RawMailStore|aws_sdk_s3|raw_mail_prefix" backend/shared/src backend/shared/Cargo.toml backend/Cargo.toml`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared raw_mail_store`
- Red before: `S3RawMailStore` does not exist. Green after: tests cover prefix
  enforcement, rejected out-of-prefix keys, PII-safe external-service errors,
  and construction from M4 mail config. The workspace build proves the concrete
  AWS SDK implementation compiles.

## Step 4 - Add inbound data contracts and confirmed ingest limits  [depends on #1]

Files:
- `backend/shared/src/inbound/mod.rs`
- `backend/shared/src/inbound/types.rs`
- `backend/shared/src/inbound/limits.rs`
- `backend/shared/src/lib.rs`

Reference behavior:
- `mail-foundation-spec.md` defines inbound message fields: headers, threading
  keys, recipients, safe text, auth results, spam/virus verdicts, attachment
  metadata, matched routing fields, S3 raw key, and status/security
  disposition.
- M2 schema defines the persistence shape in `messages`, `recipients`,
  `attachment_refs`, `threads`, and `contacts`.
- Step 1 confirms the numeric cap values used by M5.

Change:
- Add shared inbound DTOs for parsed messages, recipients, attachments,
  auth-result values, routing matches, rejected-audit records, and persisted
  message summaries.
- Add `IngestLimits` with the step 1 values as defaults and explicit
  constructors for tests.
- Represent attachment bytes as absent by type: attachment DTOs include only
  declared filename, declared content type, optional content ID, position, and
  optional declared/decoded size.
- Keep these as internal service contracts; do not expose API routes.

Verify:
- Before the change:
  `! rg "IngestLimits|ParsedInboundMessage|InboundAttachment|InboundRecipient" backend/shared/src/inbound backend/shared/src/lib.rs`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_types`
- Red before: inbound DTOs do not exist. Green after: tests cover default cap
  values, custom test limits, attachment metadata excluding bytes, recipient
  kind parsing, and status/security string values matching the M2 constraints.

## Step 5 - Add MIME parsing with hard cap enforcement  [depends on #4]

Files:
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`
- `backend/shared/src/inbound/mime.rs`
- `backend/shared/src/inbound/types.rs`
- `backend/shared/tests/fixtures/inbound/simple_text.eml`
- `backend/shared/tests/fixtures/inbound/multipart_alternative.eml`
- `backend/shared/tests/fixtures/inbound/nested_multipart.eml`
- `backend/shared/tests/fixtures/inbound/with_attachments.eml`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M5 requires MIME parser tests for MIME variants
  and ingest caps.
- `mail-foundation-spec.md` requires reject-past-limit behavior for message
  size, multipart nesting depth, and attachment count.
- ADR-0001 requires parsing logic to be testable outside Lambda handlers.

Change:
- Add a proven MIME parsing crate and wrap it behind
  `parse_raw_mime(bytes, limits)`.
- Enforce raw byte size before MIME parsing.
- Enforce multipart nesting depth and attachment count while walking the parsed
  MIME tree.
- Return typed parse/cap errors that can become rejected audit records later.
- Do not persist anything in this step.

Verify:
- Before the change:
  `! rg "parse_raw_mime|InboundParseError|LimitExceeded" backend/shared/src/inbound`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_mime`
- Red before: the parser contract does not exist. Green after: tests cover
  simple text, multipart alternative, nested multipart within limit,
  over-size raw MIME rejection, over-depth rejection, and over-attachment-count
  rejection.

## Step 6 - Add safe body text selection and HTML-to-text conversion  [depends on #5]

Files:
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`
- `backend/shared/src/inbound/text.rs`
- `backend/shared/src/inbound/mime.rs`
- `backend/shared/tests/fixtures/inbound/html_only_safe.eml`
- `backend/shared/tests/fixtures/inbound/html_only_dangerous.eml`
- `backend/shared/tests/fixtures/inbound/text_and_html_prefers_text.eml`

Reference behavior:
- `mail-foundation-spec.md` says to use usable `text/plain` when present;
  otherwise convert HTML to readable plaintext.
- `mail-foundation-spec.md` and ADR-0002 require sender HTML, CSS, scripts,
  event handlers, and remote references to be stripped from the renderable
  application path.
- Links are inert text by default; if link text is preserved, dangerous schemes
  must not become active links.

Change:
- Add `select_body_text` used by the MIME parser.
- Prefer non-empty `text/plain` over HTML.
- Add HTML-to-text fallback that strips markup, scripts, styles, event
  handlers, images/remote references, and CSS while preserving useful paragraph,
  list, and link URL text.
- Treat `javascript:`, `data:`, and other dangerous link schemes as inert text,
  not application links.
- Store no sender HTML output from this step.

Verify:
- Before the change:
  `! rg "select_body_text|html_to_text|dangerous.*link" backend/shared/src/inbound`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_text`
- Red before: body selection/conversion symbols do not exist. Green after:
  tests cover text/plain preference, HTML-only fallback, script/style removal,
  remote image/reference removal, paragraph/list preservation, safe URL text
  preservation, and `javascript:`/`data:` remaining inert plaintext.

## Step 7 - Extract inbound headers, recipients, threading keys, and attachment metadata  [depends on #5, #6]

Files:
- `backend/shared/src/inbound/mime.rs`
- `backend/shared/src/inbound/types.rs`
- `backend/shared/tests/fixtures/inbound/threaded_reply.eml`
- `backend/shared/tests/fixtures/inbound/display_name_spoof.eml`
- `backend/shared/tests/fixtures/inbound/with_attachments.eml`

Reference behavior:
- `mail-foundation-spec.md` requires extracting From address/display,
  To/Cc recipients, Subject, Date, RFC Message-ID, In-Reply-To, References,
  body text, and attachment metadata only.
- `mail-foundation-spec.md` says declared filenames are untrusted and display
  names are never identity.
- M2 schema stores normalized recipient addresses in `recipients` and
  attachment metadata in `attachment_refs`.

Change:
- Extend `parse_raw_mime` to produce a full `ParsedInboundMessage`.
- Normalize address identities to lowercase route addresses while preserving
  display names only as display strings.
- Extract `To` and `Cc` recipients from MIME headers; reserve `Bcc` for future
  outbound use and do not invent hidden inbound recipients.
- Extract RFC threading headers and normalize `References` into ordered IDs.
- Extract attachment metadata only: position, declared filename, declared
  content type, content ID, and optional size.
- Do not trust display name or attachment filename for identity, routing, or
  storage keys.

Verify:
- Before the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_mime -- --exact extracts_inbound_metadata`
  fails because the test/symbols do not exist.
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_mime`
- Red before: metadata extraction contract does not exist. Green after: tests
  cover From display/address split, To/Cc recipients, subject/date, Message-ID,
  In-Reply-To, References, attachment metadata without bytes, and a spoofed
  display name not changing normalized sender identity.

## Step 8 - Map SES auth and scan verdicts to inbound decisions  [depends on #2, #4]

Files:
- `backend/shared/src/inbound/security.rs`
- `backend/shared/src/inbound/types.rs`
- `backend/shared/src/mail_security.rs`

Reference behavior:
- `mail-foundation-spec.md` requires SES spam/virus verdicts to be enforcement
  inputs, not passive metadata.
- Current M4 `mail_security` classifies clean mail as accepted, spam/gray/
  processing-failed or missing verdicts as quarantined, and virus failures as
  rejected.
- M2 schema constrains SPF/DKIM/DMARC/auth result values and spam/virus result
  values.

Change:
- Add mapping from `InboundReceipt` SES verdict strings to typed auth results,
  scan verdicts, database strings, message status, and security reason.
- Reuse `classify_inbound_security`; do not fork security semantics.
- Compute an overall auth verdict from SPF/DKIM/DMARC for storage only; do not
  use display names as identity and do not forward/send anything in M5.
- Treat unknown or missing scan verdict strings as indeterminate quarantine,
  not accepted.

Verify:
- Before the change:
  `! rg "InboundSecurityDecision|map_ses_auth|map_ses_scan" backend/shared/src/inbound`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_security`
- Red before: inbound security mapping does not exist. Green after: tests cover
  accepted clean mail, spam quarantine, missing/unknown scan quarantine, virus
  fail rejection, auth result normalization, and database values matching M2
  check constraints.

## Step 9 - Add inbound routing resolution against domain policy  [depends on #2, #4]

Files:
- `backend/shared/src/inbound/routing.rs`
- `backend/shared/src/inbound/types.rs`
- `backend/shared/src/routing.rs`
- `backend/shared/tests/inbound_routing.rs`

Reference behavior:
- `mail-foundation-spec.md` defines per-domain `allowlist` and `catchall`
  policies and `+tag` parsing.
- M4 `routing.rs` already parses policies and route addresses.
- M4 `domain_config.rs` already represents domain/address configuration.
- Unknown recipients are rejected/dropped at ingest per policy without
  app-side bounces.

Change:
- Add an `InboundRoutingLookup` trait plus in-memory and PostgreSQL
  implementations for active domains and active accepted addresses.
- Add a resolver that evaluates SES envelope recipients, not sender-controlled
  display names.
- For allowlist domains, match the parsed base local part to active accepted
  addresses and retain the original plus-tag suffix.
- For catchall domains, accept any local part for active domains.
- When multiple configured recipients match, choose the first matching SES
  envelope recipient as the message-level matched address and still persist all
  parsed To/Cc recipients later.
- Return an explicit rejected routing decision for inactive domains, unknown
  domains, inactive addresses, malformed recipients, and allowlist misses.
- Do not generate bounces or outbound work.

Verify:
- Before the change:
  `! rg "InboundRoutingLookup|resolve_inbound_route|RoutingDecision" backend/shared/src/inbound backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_routing`
- Red before: inbound routing lookup/resolution symbols do not exist. Green
  after: tests cover allowlist acceptance, catchall acceptance, plus-tag
  retention, inactive domain rejection, inactive address rejection, unknown
  recipient rejection, malformed recipient rejection, and deterministic first
  match selection.

## Step 10 - Add threading and contact-linking primitives  [depends on #7]

Files:
- `backend/shared/src/inbound/threading.rs`
- `backend/shared/src/inbound/types.rs`
- `backend/shared/src/contacts.rs`
- `backend/shared/tests/inbound_threading.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M5 requires thread derivation and contact-linking
  primitives.
- `mail-foundation-spec.md` says threading keys come from Message-ID,
  In-Reply-To, and References.
- `mail-foundation-spec.md` says `From` display is display-only and never
  identity; contacts are linked by address, not display name.
- M2 schema stores `threads.normalized_subject`, JSON participants,
  `message.thread_id`, and nullable `message.contact_id`.

Change:
- Add helpers to normalize subjects for thread fallback by repeatedly stripping
  reply/forward prefixes, trimming whitespace, and lowercasing.
- Add a `ThreadSeed` derived from RFC threading headers, normalized subject,
  sender address, recipient addresses, and message date/received time.
- Add contact-link lookup primitives that match only normalized email address
  against `contacts.primary_address_normalized`.
- Do not create contacts automatically from inbound mail in M5.
- Do not trust display names for contact identity.

Verify:
- Before the change:
  `! rg "ThreadSeed|normalize_subject|contact.*identity" backend/shared/src/inbound backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_threading`
- Red before: threading/contact primitives do not exist. Green after: tests
  cover subject normalization, references/in-reply-to seed creation,
  participant normalization, matching a contact by normalized sender address,
  and not matching a spoofed display name to a contact.

## Step 11 - Add PostgreSQL inbound persistence and idempotency  [depends on #8, #9, #10]

Files:
- `backend/shared/src/inbound/repository.rs`
- `backend/shared/src/inbound/mod.rs`
- `backend/shared/src/db.rs`
- `backend/shared/tests/inbound_ingest_model.rs`
- `db/migrations/001_create_mail_model.sql`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M5 requires idempotency by SES message ID/S3 key.
- `mail-foundation-spec.md` says accepted, quarantined, and rejected audit
  records are written to Postgres, and non-accepted mail is excluded from the
  normal mailbox/resend path.
- M2 schema provides unique indexes on `messages.ses_message_id` and
  `messages.s3_raw_key`, normalized recipients, attachment refs, threads, and
  contacts.
- Virus failures and routing/cap rejects should persist minimal audit metadata
  only.

Change:
- Add an `InboundRepository` trait and `PgInboundRepository`.
- In a transaction, check existing `messages` by SES message ID or S3 raw key
  and return the existing summary without duplicating recipients, attachments,
  or thread counts.
- Insert accepted and quarantined inbound messages with parsed safe body text,
  auth/scan verdicts, security disposition/reason, route match, S3 raw key,
  size, recipients, attachment metadata, thread link, and contact link.
- Insert rejected audit records for virus failures, routing rejects, and cap
  rejects with minimal metadata: SES ID, S3 key when known, envelope
  recipients when known, security/routing/cap reason, status `rejected`,
  empty body text, and no attachment refs.
- Update/create threads consistently for accepted and quarantined mail only;
  rejected audits do not create user-visible conversation state.
- Do not insert `outbound_work`, forwarding rules, suppressions, or frontend
  mailbox state.

Verify:
- Before the change:
  `! rg "InboundRepository|PgInboundRepository|persist_inbound" backend/shared/src/inbound backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared --test inbound_ingest_model -- --nocapture`
- Red before: repository symbols and integration test do not exist. Green
  after: the real PostgreSQL integration test applies M2 migrations and seed
  data, persists an accepted message, persists a spam-quarantined message,
  persists a virus-rejected minimal audit without body/attachment refs, links
  an existing contact by normalized sender address, creates/updates a thread,
  and proves retrying the same SES ID/S3 key is idempotent.

## Step 12 - Add the inbound ingest orchestration service  [depends on #2, #3, #5, #8, #9, #11]

Files:
- `backend/shared/src/inbound/service.rs`
- `backend/shared/src/inbound/mod.rs`
- `backend/shared/src/ports.rs`
- `backend/shared/tests/inbound_service.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M5 defines the full flow: parse SES metadata,
  fetch raw MIME from S3, enforce caps, extract metadata/text, enforce
  spam/virus disposition, apply routing policy, and persist idempotently.
- `mail-foundation-spec.md` says non-accepted mail is never resent-forwarded
  and unknown/suspicious inbound must not generate app-side bounces.
- ADR-0001 requires business logic to be testable outside Lambda glue.

Change:
- Add `InboundIngestService` that composes `RawMailStore`,
  `InboundRoutingLookup`, `InboundRepository`, MIME parsing, security mapping,
  and ingest limits.
- Process every SES record in the Lambda event and return a summary with
  processed, accepted, quarantined, rejected, idempotent, and failed counts.
- Fetch raw MIME only after event validation.
- For parser cap failures, persist a rejected audit from SES metadata and do
  not retry as a transient failure.
- For virus failures and routing rejects, persist rejected audit records and do
  not store body text or attachment refs.
- For spam/indeterminate scans, persist quarantined records but never mark them
  eligible for normal mailbox/resend behavior.
- Do not call `MailSender`, `FeedbackPublisher`, forwarding rules, or any
  frontend/API code.

Verify:
- Before the change:
  `! rg "InboundIngestService|process_receipt_event|IngestSummary" backend/shared/src/inbound backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_service`
- Red before: orchestration service symbols do not exist. Green after: tests
  use in-memory raw mail/routing/repository doubles to cover clean accepted
  mail, spam quarantine, virus rejection with no body/attachments, unknown
  recipient rejection with no bounce/send call, cap rejection as non-transient,
  multi-record processing, raw-store transient failure, and retry idempotency.

## Step 13 - Wire the ingest Lambda to the shared service  [depends on #12]

Files:
- `backend/ingest/Cargo.toml`
- `backend/ingest/src/lib.rs`
- `backend/ingest/src/main.rs`
- `backend/shared/src/raw_mail_store.rs`

Reference behavior:
- M4 left `ingest::handle_event` as a stable no-op scaffold.
- `MAIL-FOUNDATION-PLAN.md` M5 requires the ingest Lambda to persist safe text
  mail from SES events.
- `../ahara/INTEGRATION.md` provides runtime DB settings through environment
  variables and the platform migration flow owns schema changes.
- ADR-0001 says Lambda binaries should remain thin glue around testable shared
  logic.

Change:
- Replace the no-op `handle_event` implementation with a thin call into
  `InboundIngestService`.
- Keep a testable function that accepts injected raw-mail/routing/repository
  dependencies for unit tests.
- In `main.rs`, load `AppConfig`, initialize tracing, create the PostgreSQL
  pool, create the S3 raw-mail store, create PostgreSQL routing/repository
  implementations, and run the shared service.
- Return a stable JSON summary; do not include message bodies, raw MIME,
  attachment filenames beyond summary counts, or full headers in the Lambda
  response/logs.

Verify:
- Before the change:
  `cargo test --manifest-path backend/Cargo.toml -p ingest ingest_handler -- --exact handler_persists_clean_ses_receipt`
  fails because the handler still returns the scaffold response and the test
  does not exist.
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p ingest`
- Red before: the ingest handler has no service wiring contract. Green after:
  tests cover a clean SES receipt summary, a rejected virus receipt summary,
  missing/invalid SES event errors, and no body/header leakage in the response.

## Step 14 - Run the M5 exit gate  [depends on #13]

Files:
- No implementation files unless an earlier verification exposes a build-only
  wiring miss directly caused by M5.

Reference behavior:
- M5 exit in `MAIL-FOUNDATION-PLAN.md` requires `make ci` green and parser
  tests covering MIME variants, HTML stripping, dangerous links,
  spam/virus disposition, retry idempotency, routing policy, and ingest caps.
- M5 scope guard forbids DNS routability, frontend mailbox behavior, outbound
  behavior, and feedback handling changes.

Change:
- Run the focused M5 tests first, then the repository-wide gate.
- If a focused test fails, fix only the M5-scoped cause in the files named by
  the failed step.
- Do not continue into M6.

Verify:
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p shared --test inbound_ingest_model -- --nocapture`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p ingest`
- Run:
  `make ci`
- Run:
  `! rg -n "type *= *\"MX\"|MX[[:space:]]+|active *= *true|enabled *= *true" infrastructure/terraform/mail_receiving.tf infrastructure/terraform`
- Run:
  `! rg -n "outbound_work|send_mail\\(|MailSender|FeedbackPublisher|/messages|/threads|mailbox|compose|forward" backend/ingest/src backend/shared/src/inbound frontend/src`
- The executor reports the actual output for the focused tests and `make ci`,
  then stops at the end of M5.
