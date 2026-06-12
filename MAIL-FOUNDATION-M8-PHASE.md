# M8 - Operational Hardening And Release Readiness

This phase expands only M8 from `MAIL-FOUNDATION-PLAN.md`. The goal is to
close production-readiness gaps after the mail flows exist: PII-safe
observability, operational metrics and alarms, inbound flood controls, Lambda
blast-radius controls, least-privilege/S3 hardening review, deploy
documentation, CI/deploy alignment, active address-scoped SES receiving, MX,
and a controlled smoke procedure.

M8 scope guard:
- Do not add new mail product features. Attachment handling, IMAP/mobile,
  calendar/bookings/money, arbitrary forwarding, and vector search remain
  backlog items.
- Do not change mailbox, routing, compose, forwarding, or feedback semantics
  except where observability or flood-control wiring records existing outcomes.
- Do not log message bodies, full headers, subjects, raw MIME keys, unredacted
  email addresses, attachment filenames, or recipient lists.
- Do not broaden IAM grants. Any IAM change must reduce scope, preserve current
  scoped resources, or add only the specific Lambda/CloudWatch/SES receiving
  controls named by this phase.
- Do not use domain-wide SES receipt routing. Active SES receipt recipients are
  explicit accepted addresses only.
- Do not add provisioned concurrency. Reserved concurrency is allowed because
  AWS documents that it has no additional charge.
- Do not start local dev servers.

Reference context:
- `MAIL-FOUNDATION-PLAN.md`
- `mail-foundation-spec.md`
- `MAIL-FOUNDATION-M3-PHASE.md`
- `MAIL-FOUNDATION-M4-PHASE.md`
- `MAIL-FOUNDATION-M5-PHASE.md`
- `MAIL-FOUNDATION-M6-PHASE.md`
- `MAIL-FOUNDATION-M7-PHASE.md`
- `docs/adr/0001-rust-lambda-and-react-spa.md`
- `docs/adr/0002-public-authenticated-text-only-ui.md`
- `docs/adr/0003-shared-cognito-strong-auth.md`
- `docs/adr/0004-project-owned-mail-infrastructure.md`
- `../ahara/INTEGRATION.md`
- `../ahara/CI-WORKFLOW.md`
- Shared Lambda and ALB modules in `../ahara-tf-patterns/modules/lambda/`
  and `../ahara-tf-patterns/modules/alb-api/`
- Current Terraform in `infrastructure/terraform/`
- Current Lambda handlers in `backend/api/`, `backend/ingest/`,
  `backend/send-worker/`, and `backend/feedback-handler/`

Exit gate:
- Focused observability tests prove runtime metric payloads and handler
  responses contain counts/status only and omit mail content and unredacted
  addresses.
- Focused flood-control tests prove unknown recipients are stopped before S3,
  oversized raw objects are rejected before raw body fetch, and rolling raw-byte
  caps reject without normal mailbox/forwarding side effects.
- Terraform validates explicit SES recipient routing, MX activation, Lambda
  concurrency caps, Lambda/app metric alarms, S3 public-access/encryption
  controls, and scoped IAM.
- Deploy docs, `.env.example`, CI alignment, and smoke procedure are present
  and reference the platform workflow.
- `make ci` is green.
- `make build` is green.
- Terraform deploy succeeds.
- Post-deploy checks confirm the active receipt rule set and MX record.
- Source scans confirm no PII logging patterns, no broad IAM grants, active
  receiving is address-scoped, and S3 hardening primitives remain present.

## Step 1 - Record operational defaults and release activation mode  [DECISION]

Files:
- `MAIL-FOUNDATION-M8-PHASE.md`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M8 requires operational hardening and release
  readiness.
- `mail-foundation-spec.md` requires Lambda concurrency caps, PII-safe logs,
  bounce/complaint alarms, S3 public-access/SSE controls, least-privilege IAM,
  and cost/blast-radius controls.
- AWS SES classic receipt rules do not provide a custom rolling attachment-size
  cap. The M8 control shape is explicit recipient conditions before S3, a
  pre-S3 gate for metadata-only throttling, and post-S3 `HeadObject` raw-size
  checks before raw MIME download.

Change:
- Record these confirmed M8 defaults:
  - Activate inbound routing in this phase by adding MX, setting the project
    SES receipt rule set active, and enabling the receipt rule.
  - Keep receipt routing address-scoped: SES receipt rule recipients are
    explicit accepted addresses only, matching the seed routing addresses
    `chris@ahara.io` and `contact@ahara.io`.
  - Add a pre-S3 synchronous SES gate Lambda. It enforces recipient and
    message-count controls from SES event metadata, then returns
    `STOP_RULE_SET` or `CONTINUE`; it must not fetch or log raw MIME.
  - Add post-S3 raw-size controls before raw MIME fetch/parse: reject raw
    objects over `10 MiB`, and reject once accepted/rejected inbound raw bytes
    in the previous rolling hour plus the current object exceeds `50 MiB`.
  - Receipt gate limits: `120` messages per accepted recipient per rolling
    hour and `240` total accepted-recipient messages per rolling hour.
  - Lambda reserved concurrency caps: API `20`, receipt gate `2`, ingest `5`,
    send worker `2`, feedback handler `2`.
  - App metric namespace: `AharaBusiness/Mail`.
  - Alarm thresholds:
    - any Lambda `Errors >= 1` in one 5-minute period for API, receipt gate,
      ingest, send worker, or feedback handler,
    - any Lambda `Throttles >= 1` in one 5-minute period for API, receipt
      gate, ingest, send worker, or feedback handler,
    - any app metric `InboundFailed >= 1`, `OutboundFailed >= 1`,
      `FeedbackComplained >= 1`, `InboundGateBlocked >= 1`,
      `InboundOversizeRejected >= 1`, or `InboundHourlyBytesRejected >= 1`
      in one 5-minute period,
    - keep existing SES reputation thresholds at bounce rate `0.05` and
      complaint rate `0.001`.
  - Alarm delivery remains the existing `mail_alarms` SNS topic; subscriptions
    are documented as an operator step rather than hardcoding a personal email
    target in Terraform.

Verify:
- No automated verification. The executor records that the user confirmed MX
  activation and reserved concurrency, then proceeds to step 2.

## Step 2 - Add PII-safe observability helpers

Files:
- `backend/shared/src/observability.rs`
- `backend/shared/src/lib.rs`
- `backend/shared/Cargo.toml`
- `backend/Cargo.toml`

Reference behavior:
- `mail-foundation-spec.md` says logs must not include full headers or bodies
  and must scrub PII.
- ADR-0001 keeps shared Lambda logic in library crates so policy can be tested
  outside handler glue.
- Existing handlers already return count summaries without body/header content;
  M8 adds a reusable contract so future handlers do not hand-roll metrics.

Change:
- Add a shared observability module with:
  - a PII-safe email redaction/hash helper for cases where a stable identifier
    is needed without logging the address,
  - a CloudWatch Embedded Metric Format JSON builder using the confirmed
    namespace,
  - helpers for count-only metric dimensions: service name, handler name, and
    mail domain only,
  - tests that reject metric payloads containing body text, full headers,
    subjects, raw MIME keys, recipient lists, attachment filenames, or
    unredacted email addresses.
- Update `init_tracing` only if needed to keep JSON structured logging and add
  stable service/runtime fields. Do not change logging to plaintext.

Verify:
- Before the change:
  `! rg "observability|EmbeddedMetric|AharaBusiness/Mail|redact_email_for_log" backend/shared/src backend/shared/Cargo.toml backend/Cargo.toml`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared observability`
- Red before: the shared observability symbols and tests do not exist. Green
  after: helpers produce EMF-compatible JSON and tests prove unsafe mail
  content is omitted/redacted.

## Step 3 - Add inbound flood-control contracts  [depends on #1, #2]

Files:
- `backend/shared/src/inbound/limits.rs`
- `backend/shared/src/inbound/repository.rs`
- `backend/shared/src/inbound/service.rs`
- `backend/shared/src/ports.rs`
- `backend/shared/src/raw_mail_store.rs`
- `backend/shared/tests/inbound_service.rs`
- `backend/shared/tests/inbound_ingest_model.rs`

Reference behavior:
- `mail-foundation-spec.md` requires ingest caps, Lambda cost/blast-radius
  controls, and no normal mailbox persistence for rejected mail.
- SES classic receipt rules cannot enforce a custom lower attachment cap before
  S3; M8 therefore uses pre-S3 message-count controls and post-S3 raw object
  size checks before fetching/parsing raw MIME.
- M5 already rejects oversized raw MIME after fetching. M8 moves the cheap S3
  size decision before body download and adds a rolling raw-byte cap.

Change:
- Extend ingest limits with confirmed M8 values:
  - `max_raw_mail_object_bytes = 10 MiB`,
  - `max_recent_raw_mail_bytes = 50 MiB`,
  - `recent_raw_mail_window_seconds = 3600`.
- Extend `RawMailStore` with a metadata/head operation that returns raw object
  size without downloading bytes. Implement it for S3 using `HeadObject` and
  for in-memory test doubles.
- Extend `InboundRepository` with a rolling raw-byte total over recent inbound
  audit/message rows. Implement it for Postgres using `messages.size_bytes`
  and `COALESCE(received_at, created_at)`.
- In `InboundIngestService`, before fetching raw MIME bytes:
  - `head` the raw object,
  - persist a minimal rejected audit if the object exceeds the per-message cap,
  - persist a minimal rejected audit if the rolling byte cap would be exceeded,
  - emit count-only metrics for oversize/hourly rejections.
- Do not parse body, extract attachments, forward, or create normal mailbox
  rows for these flood-control rejections.

Verify:
- Before the change:
  `! rg "max_raw_mail_object_bytes|recent_raw_mail_window|raw_mail_size|InboundOversizeRejected|InboundHourlyBytesRejected" backend/shared/src backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_limits`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_service`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared --test inbound_ingest_model`
- Red before: limit fields, raw metadata, rolling byte repository contract,
  and tests do not exist. Green after: oversized/hourly-cap messages are
  rejected before raw body fetch and persisted as minimal audit rows only.

## Step 4 - Add the pre-S3 SES receipt gate Lambda  [depends on #1, #2]

Files:
- `backend/Cargo.toml`
- `backend/receipt-gate/Cargo.toml`
- `backend/receipt-gate/src/lib.rs`
- `backend/receipt-gate/src/main.rs`
- `backend/shared/src/inbound/receipt_gate.rs`
- `backend/shared/src/inbound/mod.rs`
- `platform.yml`

Reference behavior:
- AWS SES synchronous Lambda receipt actions can control mail flow with
  `STOP_RULE`, `STOP_RULE_SET`, or `CONTINUE`; the event contains headers and
  verdict metadata but not the body.
- `mail-foundation-spec.md` avoids app-side bounces and prefers allowlist
  routing to limit dictionary attack volume.

Change:
- Add shared receipt-gate logic that:
  - parses SES receipt events without needing S3 object keys,
  - allows only confirmed accepted recipients plus their plus-address variants,
  - enforces count limits of `120` messages per accepted recipient per rolling
    hour and `240` total accepted-recipient messages per rolling hour,
  - returns `STOP_RULE_SET` for unknown recipients or exceeded counts and
    `CONTINUE` otherwise,
  - emits only count/status metrics and never logs body/header content,
    subjects, sender addresses, recipient lists, or raw keys.
- Add a `receipt-gate` Lambda crate using the shared logic.
- Register the crate in the Rust workspace and `platform.yml` Lambda
  artifacts.
- Do not fetch S3, parse MIME, persist mailbox rows, or send mail from this
  Lambda.

Verify:
- Before the change:
  `! rg "receipt-gate|ReceiptGate|STOP_RULE_SET|InboundGateBlocked" backend platform.yml`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared receipt_gate`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p receipt-gate`
- Red before: receipt gate symbols/crate do not exist. Green after: tests cover
  accepted recipients, plus-address variants, unknown-recipient blocking, rate
  blocking, SES disposition shape, and no PII/content leakage.

## Step 5 - Wire count-only metrics into mail Lambdas  [depends on #2, #3, #4]

Files:
- `backend/ingest/src/lib.rs`
- `backend/send-worker/src/lib.rs`
- `backend/feedback-handler/src/lib.rs`
- `backend/api/src/main.rs`
- `backend/receipt-gate/src/lib.rs`
- `backend/ingest/Cargo.toml`
- `backend/send-worker/Cargo.toml`
- `backend/feedback-handler/Cargo.toml`
- `backend/api/Cargo.toml`
- `backend/receipt-gate/Cargo.toml`

Reference behavior:
- M5-M7 handlers already summarize accepted/quarantined/rejected, send
  outcomes, and feedback outcomes.
- `mail-foundation-spec.md` requires PII-safe logs and operational visibility;
  observability must not change persistence, routing, sending, forwarding, or
  feedback semantics.

Change:
- Emit one count-only app metric payload per handler invocation:
  - ingest: `InboundProcessed`, `InboundAccepted`, `InboundQuarantined`,
    `InboundRejected`, `InboundFailed`, `InboundOversizeRejected`,
    `InboundHourlyBytesRejected`,
  - receipt gate: `InboundGateProcessed`, `InboundGateAllowed`,
    `InboundGateBlocked`,
  - send worker: `OutboundClaimed`, `OutboundSent`, `OutboundRetried`,
    `OutboundFailed`, `OutboundSuppressed`,
  - feedback handler: `FeedbackProcessed`, `FeedbackBounced`,
    `FeedbackComplained`, `FeedbackSuppressedRecipients`,
  - API startup/health only if a count-only API metric is needed for alarm
    dimensions; do not log request payloads.
- Keep existing tracing fields count-only. Do not add address, subject, body,
  attachment filename, header, or raw S3 key fields.
- Add handler tests that exercise existing summaries and assert the serialized
  response/metric payloads do not contain fixture body text, sender addresses,
  recipient addresses, subjects, attachment filenames, or raw MIME keys.

Verify:
- Before the change:
  `! rg "InboundProcessed|OutboundSent|FeedbackComplained|InboundGateBlocked|InboundOversizeRejected|emit_mail_metric" backend/ingest/src backend/send-worker/src backend/feedback-handler/src backend/api/src backend/receipt-gate/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p ingest operational`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p send-worker operational`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p feedback-handler operational`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p receipt-gate operational`
- Red before: metric emission symbols/tests do not exist. Green after: handler
  tests pass and prove emitted operational payloads are count-only.

## Step 6 - Expose and apply Lambda reserved concurrency caps  [depends on #1, #4]

Files:
- `../ahara-tf-patterns/modules/lambda/variables.tf`
- `../ahara-tf-patterns/modules/lambda/main.tf`
- `../ahara-tf-patterns/modules/alb-api/variables.tf`
- `../ahara-tf-patterns/modules/alb-api/main.tf`
- `infrastructure/terraform/locals.tf`
- `infrastructure/terraform/lambdas.tf`
- `infrastructure/terraform/outputs.tf`

Reference behavior:
- `mail-foundation-spec.md` requires Lambda concurrency caps on ingest and send
  worker to bound cost and blast radius.
- ADR-0004 keeps project mail infrastructure in project Terraform while reusing
  shared platform modules.
- The shared `lambda` module currently owns `aws_lambda_function`, so reserved
  concurrency must be exposed there and threaded through `alb-api` for the API
  Lambda.

Change:
- Add an optional `reserved_concurrent_executions` input to the shared Lambda
  module and pass it to `aws_lambda_function.this`.
- Add an optional per-Lambda `reserved_concurrent_executions` field to the
  shared `alb-api` module and pass it through to the internal Lambda module.
- Add project locals for the confirmed M8 concurrency caps and apply them to
  API, receipt gate, ingest, send worker, and feedback handler Lambdas.
- Output the effective caps if useful for release review; do not change
  timeout, memory, routes, or auth settings unless required by the module
  input wiring.

Verify:
- Before the change:
  `! rg "reserved_concurrent_executions|lambda_reserved_concurrency" ../ahara-tf-patterns/modules/lambda ../ahara-tf-patterns/modules/alb-api infrastructure/terraform`
- After the change:
  `terraform fmt -check -recursive ../ahara-tf-patterns/modules/lambda ../ahara-tf-patterns/modules/alb-api infrastructure/terraform/`
- After the change:
  `rg "reserved_concurrent_executions" ../ahara-tf-patterns/modules/lambda ../ahara-tf-patterns/modules/alb-api infrastructure/terraform`
- Red before: no module or project concurrency cap input exists. Green after:
  Terraform formatting passes and the caps are present at shared-module and
  project call sites.

## Step 7 - Activate address-scoped SES receiving and MX  [depends on #1, #4, #6]

Files:
- `infrastructure/terraform/locals.tf`
- `infrastructure/terraform/lambdas.tf`
- `infrastructure/terraform/mail_receiving.tf`
- `infrastructure/terraform/mail_ses.tf`
- `infrastructure/terraform/mail_iam.tf`
- `infrastructure/terraform/outputs.tf`

Reference behavior:
- `mail-foundation-spec.md` says allowlist is preferred over catch-all to
  limit dictionary attack volume.
- AWS SES recipient conditions can match explicit addresses and plus-address
  variants before rule actions run.
- The pre-S3 gate must run before S3 storage and ingest processing.

Change:
- Add locals for confirmed accepted SES recipients:
  `chris@ahara.io` and `contact@ahara.io`.
- Add the `receipt-gate` Lambda module with common environment and the shared
  Lambda role/policy.
- Change `aws_ses_receipt_rule.raw_mail_ingest` to:
  - `recipients = local.accepted_mail_recipients`,
  - `enabled = true`,
  - first action: synchronous `lambda_action` to `receipt-gate` with
    `invocation_type = "RequestResponse"`,
  - second action: S3 write,
  - third action: async ingest Lambda.
- Add `aws_ses_active_receipt_rule_set` for the project rule set.
- Add an MX Route53 record for `ahara.io` pointing to the SES inbound endpoint
  for the configured AWS region.
- Do not add domain-wide receipt recipients or catch-all routing.

Verify:
- Before the change:
  `! rg "receipt_gate|accepted_mail_recipients|aws_ses_active_receipt_rule_set|type\\s*=\\s*\\\"MX\\\"|enabled\\s*=\\s*true|RequestResponse" infrastructure/terraform`
- After the change:
  `terraform fmt -check -recursive infrastructure/terraform/`
- After the change:
  `terraform -chdir=infrastructure/terraform init -backend=false`
- After the change:
  `terraform -chdir=infrastructure/terraform validate`
- After the change:
  `rg "accepted_mail_recipients|chris@ahara.io|contact@ahara.io|aws_ses_active_receipt_rule_set|RequestResponse|type\\s*=\\s*\\\"MX\\\"" infrastructure/terraform`
- After the change:
  `! rg -n "recipients\\s*=\\s*\\[local\\.mail_domain\\]|recipients\\s*=\\s*\\[local\\.domain_name\\]" infrastructure/terraform -g '*.tf'`
- Red before: Terraform has domain-wide disabled receipt routing and no MX.
  Green after: Terraform validates with address-scoped active receiving and MX.

## Step 8 - Add Lambda and app metric alarms  [depends on #1, #5, #6, #7]

Files:
- `infrastructure/terraform/locals.tf`
- `infrastructure/terraform/mail_alarms.tf`
- `infrastructure/terraform/outputs.tf`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M8 requires structured metrics and alarms.
- `mail-foundation-spec.md` calls out bounce/complaint rates, Lambda
  blast-radius controls, and operational/cost controls.
- M3 already created SES reputation alarms and an alarm SNS topic; M8 extends
  this rather than replacing it.

Change:
- Keep existing SES bounce/complaint reputation alarms.
- Add CloudWatch Lambda `Errors` and `Throttles` alarms for API, receipt gate,
  ingest, send worker, and feedback handler using the confirmed thresholds.
- Add CloudWatch app metric alarms against the confirmed namespace for
  `InboundFailed`, `OutboundFailed`, `FeedbackComplained`,
  `InboundGateBlocked`, `InboundOversizeRejected`, and
  `InboundHourlyBytesRejected`.
- Use the existing `aws_sns_topic.mail_alarms` action target. Do not add a
  personal email subscription in Terraform.
- Do not alarm on message content, subjects, raw keys, recipients, or any
  high-cardinality sender-controlled values.

Verify:
- Before the change:
  `! rg "lambda_errors|lambda_throttles|InboundFailed|OutboundFailed|FeedbackComplained|InboundGateBlocked|InboundOversizeRejected|InboundHourlyBytesRejected|AharaBusiness/Mail" infrastructure/terraform/mail_alarms.tf infrastructure/terraform/locals.tf`
- After the change:
  `terraform fmt -check -recursive infrastructure/terraform/`
- After the change:
  `terraform -chdir=infrastructure/terraform init -backend=false`
- After the change:
  `terraform -chdir=infrastructure/terraform validate`
- Red before: the Lambda/app metric alarm resources do not exist. Green after:
  Terraform initializes with local backend disabled, validates, and the alarm
  resources target only count metrics.

## Step 9 - Document the S3, IAM, and inbound flood-control hardening review

Files:
- `docs/operations.md`
- `docs/README.md`
- `README.md`
- `infrastructure/terraform/mail_storage.tf`
- `infrastructure/terraform/mail_iam.tf`
- `infrastructure/terraform/mail_feedback.tf`
- `infrastructure/terraform/mail_receiving.tf`

Reference behavior:
- ADR-0004 requires project-owned mail infrastructure with scoped IAM.
- `mail-foundation-spec.md` requires S3 public-access blocks, SSE at rest, TLS
  in transit, secrets outside code, and IAM scoped to SES identities, S3
  prefixes, SNS topics, and Lambda names.
- Current Terraform already has raw-mail public-access block and SSE resources;
  M8 records and verifies the review instead of duplicating resources.

Change:
- Add an operations document section for hardening review that lists:
  - raw MIME bucket public-access block,
  - raw MIME bucket server-side encryption,
  - lifecycle/retention controls,
  - SES write policy constrained by source account and raw prefix,
  - Lambda raw-mail access constrained to the raw prefix,
  - SES send permission constrained to the project SES identity,
  - SNS feedback publish permission constrained to feedback topics and source
    account,
  - address-scoped SES receipt recipients,
  - pre-S3 receipt gate behavior and its limits,
  - post-S3 raw-object size and rolling-byte caps,
  - MX and active receipt rule set activation.
- If the review exposes a missing hardening primitive, fix only that primitive
  in the named Terraform file.
- Link the operations document from `README.md` and `docs/README.md`.

Verify:
- Before the change:
  `! rg "S3 and IAM hardening review|raw MIME bucket public-access block|pre-S3 receipt gate|rolling-byte cap|MX activation" docs README.md`
- After the change:
  `rg "aws_s3_bucket_public_access_block|aws_s3_bucket_server_side_encryption_configuration|aws_s3_bucket_lifecycle_configuration" infrastructure/terraform/mail_storage.tf`
- After the change:
  `rg "AllowSesReceiptRuleWrite|ListRawMailPrefix|ReadWriteRawMailObjects|SendFromMailIdentity|AllowSesFeedbackPublish" infrastructure/terraform/mail_storage.tf infrastructure/terraform/mail_iam.tf infrastructure/terraform/mail_feedback.tf`
- After the change:
  `! rg -n "\"(s3|ses|sns|lambda|iam|cloudwatch):\\*\"|resources\\s*=\\s*\\[\"\\*\"\\]" infrastructure/terraform -g '*.tf'`
- Red before: the documented hardening review does not exist. Green after:
  docs are linked, required hardening primitives are present, and broad grants
  are absent.

## Step 10 - Add deploy documentation and environment example

Files:
- `.env.example`
- `docs/deploy.md`
- `docs/README.md`
- `README.md`
- `scripts/README.md`

Reference behavior:
- `../ahara/INTEGRATION.md` requires the standard project structure, shared
  state bucket, local deploy script, shared ALB, shared Cognito, shared RDS,
  and platform migration flow.
- `../ahara/CI-WORKFLOW.md` says local `scripts/deploy.sh` is a local-only
  convenience script and CI must replicate deploy steps through the shared
  workflow.
- `mail-foundation-spec.md` keeps secrets in parameter/secret stores and
  prefers role-based credentials.

Change:
- Add `.env.example` with placeholder local/runtime variable names only:
  database variables, Cognito variables, API/app base URLs, mail domain,
  raw-mail S3 config, SES feedback topic ARNs, accepted SES recipients,
  flood-control limits, `RUST_LOG`, and Terraform state overrides. Do not
  include real secrets or personal email addresses.
- Add deploy documentation that covers:
  - local prerequisites,
  - `make ci`,
  - `make build`,
  - platform migrations,
  - Terraform init/validate/plan/apply,
  - how this differs from CI deploy,
  - required AWS role/credential assumptions,
  - how to read Terraform outputs needed by the smoke procedure.
- Link the deploy document from `README.md`, `docs/README.md`, and
  `scripts/README.md`.

Verify:
- Before the change:
  `test ! -f .env.example && test ! -f docs/deploy.md`
- After the change:
  `rg "DB_HOST|COGNITO_USER_POOL_ID|MAIL_DOMAIN|RAW_MAIL_BUCKET|SES_BOUNCE_TOPIC_ARN|ACCEPTED_MAIL_RECIPIENTS|MAX_RAW_MAIL_OBJECT_BYTES|RUST_LOG" .env.example`
- After the change:
  `rg "make ci|make build|terraform plan|db-migrate|shared workflow|scripts/deploy.sh" docs/deploy.md README.md docs/README.md scripts/README.md`
- Red before: the environment example and deploy document do not exist. Green
  after: docs and placeholder env variables are present and linked.

## Step 11 - Align CI and local integration checks with platform workflow

Files:
- `.github/workflows/ci.yml`
- `platform.yml`
- `Makefile`
- `scripts/run-backend-integration-tests.sh`
- `scripts/README.md`

Reference behavior:
- `../ahara/CI-WORKFLOW.md` says standard projects use the shared reusable
  workflow, `platform.yml` declares stack and deployable Rust artifacts, and
  `Makefile ci` mirrors shared lint/test checks.
- The shared workflow supports `rust_extra_ci_commands` for repo-specific Rust
  checks that need to run inside the shared Rust cache/build topology.
- M2-M7 added PostgreSQL-backed shared integration tests beyond the original
  `mail_model` test.

Change:
- Add a standard `scripts/run-backend-integration-tests.sh` that runs the
  shared PostgreSQL-backed integration tests for mail model, inbound ingest,
  mailbox, outbound, forwarding, and feedback. Keep it parameterless and
  local/CI-safe.
- Update `Makefile` so local `make ci` uses the same integration-test script
  instead of hardcoding only one integration test.
- Update `.github/workflows/ci.yml` to pass the script through
  `rust_extra_ci_commands` while continuing to use
  `chris-arsenault/ahara/.github/workflows/ci.yml@main`.
- Confirm `platform.yml` declares the Rust Lambda artifacts:
  `api`, `receipt-gate`, `ingest`, `send-worker`, and `feedback-handler`.
- Do not replace the shared workflow with a custom CI implementation.

Verify:
- Before the change:
  `! rg "run-backend-integration-tests|rust_extra_ci_commands|outbound_model|forwarding_model|feedback_model" .github/workflows/ci.yml Makefile scripts`
- After the change:
  `bash -n scripts/run-backend-integration-tests.sh`
- After the change:
  `rg "rust_extra_ci_commands|run-backend-integration-tests.sh" .github/workflows/ci.yml Makefile scripts/README.md`
- After the change:
  `rg "api|receipt-gate|ingest|send-worker|feedback-handler" platform.yml`
- After the change:
  `scripts/run-backend-integration-tests.sh`
- Red before: the repo-specific integration script and CI wiring do not exist.
  Green after: shell syntax passes, CI/local hooks reference the same script,
  and the script passes.

## Step 12 - Add controlled smoke procedure and first-domain DNS verification guide

Files:
- `docs/smoke-check.md`
- `docs/deploy.md`
- `docs/operations.md`
- `docs/README.md`
- `README.md`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M8 exit requires the smoke procedure to verify
  receive, read, send, forward, and bounce/complaint handling in a controlled
  domain.
- `mail-foundation-spec.md` requires no app-side bounces for suspicious or
  unknown inbound mail, only clean accepted mail shown/forwarded, SES
  bounce/complaint suppression, and no HTML rendering.
- This phase activates inbound routing only for explicit accepted addresses.

Change:
- Add a smoke-check document with operator steps for:
  - preflight: Terraform outputs, Cognito access, database migration state,
    alarm topic subscription status, SES identity/DKIM verification, active
    receipt rule set, address-scoped receipt recipients, and MX record,
  - controlled receive: send one clean test message to an allowlisted address,
    verify raw S3 object and accepted mailbox row,
  - flood controls: send/construct unknown-recipient, over-size, and hourly-cap
    test events in a controlled way and verify gate/ingest rejection metrics,
  - read: open the authenticated UI and verify plaintext-only body, real sender,
    auth verdicts, read/unread, contact link, and no HTML execution,
  - send: compose text-only mail to a controlled recipient and verify queued,
    sent, and SES provider message id state,
  - forward: create one address-scoped forwarding rule, send clean pass-auth
    inbound mail, verify exactly one outbound forward with `Reply-To` original
    sender, then remove/deactivate the rule,
  - bounce/complaint: publish controlled SNS-shaped bounce/complaint events or
    use SES simulator where available, verify suppression rows and outbound
    status update,
  - rollback: disable receipt rule or remove MX if needed, remove test
    forwarding rule, confirm alarm state, and preserve logs without message
    bodies.
- Add first-domain DNS verification steps for SES TXT/DKIM, MX, active receipt
  rule set, and rollback/deactivation.
- Link the smoke guide from deploy/operations docs and indexes.

Verify:
- Before the change:
  `test ! -f docs/smoke-check.md`
- After the change:
  `rg "receive|read|send|forward|bounce|complaint|suppression|DKIM|MX|receipt rule|flood" docs/smoke-check.md`
- After the change:
  `rg "smoke-check|first-domain DNS" docs/deploy.md docs/operations.md docs/README.md README.md`
- After the change:
  `rg -n "type\\s*=\\s*\\\"MX\\\"|enabled\\s*=\\s*true|aws_ses_active_receipt_rule_set|accepted_mail_recipients" infrastructure/terraform -g '*.tf'`
- Red before: the smoke procedure does not exist. Green after: it covers the
  required workflows and Terraform is explicitly active only for accepted
  recipients.

## Step 13 - Run the M8 exit gate and deploy  [depends on #12]

Files:
- No implementation files unless an earlier verification exposes a build-only
  wiring miss directly caused by M8.

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M8 exit requires `make ci` green, Terraform plan
  reviewed, and a smoke procedure covering receive, read, send, forward, and
  bounce/complaint handling in a controlled domain.
- `mail-foundation-spec.md` requires PII-safe logs, least-privilege IAM, S3
  public-access/SSE controls, Lambda cost/blast-radius controls, inbound flood
  controls, and address-scoped active receiving.

Change:
- Run the focused M8 tests first, then the repository-wide gate, then build,
  Terraform dry checks, full deployment, and post-deploy DNS/SES checks.
- If a focused test fails, fix only the M8-scoped cause in the files named by
  the failed step.
- Do not continue beyond M8.

Verify:
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p shared observability`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p shared receipt_gate`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_service`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p receipt-gate`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p ingest operational`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p send-worker operational`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p feedback-handler operational`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p receipt-gate operational`
- Run:
  `terraform fmt -check -recursive ../ahara-tf-patterns/modules/lambda ../ahara-tf-patterns/modules/alb-api infrastructure/terraform/`
- Run:
  `terraform -chdir=infrastructure/terraform init -backend=false`
- Run:
  `terraform -chdir=infrastructure/terraform validate`
- Run:
  `make ci`
- Run:
  `make build`
- Run:
  `terraform -chdir=infrastructure/terraform plan -refresh=false -out=/tmp/ahara-business-m8.tfplan`
- Run:
  `scripts/deploy.sh`
- Run:
  `terraform -chdir=infrastructure/terraform output`
- Run:
  `aws ses describe-active-receipt-rule-set --region us-east-1`
- Run:
  `aws route53 list-resource-record-sets --hosted-zone-id "$(terraform -chdir=infrastructure/terraform output -raw route53_zone_id)" --query "ResourceRecordSets[?Name == 'ahara.io.' && Type == 'MX']"`
- Run:
  `! rg -n "tracing::(info|warn|error|debug)!.*(body_text|from_address|subject|recipient|raw_mail|s3_raw_key|header|attachment)" backend/api/src backend/ingest/src backend/receipt-gate/src backend/send-worker/src backend/feedback-handler/src backend/shared/src`
- Run:
  `! rg -n "\"(s3|ses|sns|lambda|iam|cloudwatch):\\*\"|resources\\s*=\\s*\\[\"\\*\"\\]" infrastructure/terraform ../ahara-tf-patterns/modules/lambda ../ahara-tf-patterns/modules/alb-api -g '*.tf'`
- Run:
  `rg -n "block_public_acls\\s*=\\s*true|block_public_policy\\s*=\\s*true|ignore_public_acls\\s*=\\s*true|restrict_public_buckets\\s*=\\s*true|sse_algorithm\\s*=\\s*\"AES256\"" infrastructure/terraform/mail_storage.tf`
- Run:
  `rg -n "reserved_concurrent_executions|InboundFailed|OutboundFailed|FeedbackComplained|InboundGateBlocked|InboundOversizeRejected|InboundHourlyBytesRejected|AharaBusiness/Mail" infrastructure/terraform ../ahara-tf-patterns/modules/lambda ../ahara-tf-patterns/modules/alb-api -g '*.tf'`
- Run:
  `rg -n "type\\s*=\\s*\\\"MX\\\"|enabled\\s*=\\s*true|aws_ses_active_receipt_rule_set|accepted_mail_recipients" infrastructure/terraform -g '*.tf'`
- Run:
  `! rg -n "recipients\\s*=\\s*\\[local\\.mail_domain\\]|recipients\\s*=\\s*\\[local\\.domain_name\\]" infrastructure/terraform -g '*.tf'`
- The executor reports actual output for focused tests, `make ci`,
  `make build`, Terraform validate/plan/deploy, DNS/SES checks, and scans,
  explains any broad-scan false positives with file/line references, then
  stops at the end of M8.
