# M7 - Compose, Sending, Forwarding, And Feedback

This phase expands only M7 from `MAIL-FOUNDATION-PLAN.md`. The goal is to
complete outbound workflows over the M2 storage model: authenticated text-only
compose/reply, asynchronous SES sending with retry/status, configured
resend-forwarding through the same outbound path, and SES bounce/complaint
feedback handling with suppression.

M7 scope guard:
- Do not add outbound attachments. MVP outbound bodies are text/plain only.
- Do not render or send sender-controlled HTML. Compose/reply bodies are plain
  text; forwarded message context is generated text only.
- Do not send synchronously from API routes. API routes enqueue work and return
  queued state.
- Do not bypass the outbound work table for compose, reply, or forwarding.
- Do not forward quarantined/rejected inbound mail, unknown-recipient audit
  rows, or inbound mail with failed/non-pass SPF/DKIM/DMARC/auth verdicts.
- Do not generate app-side bounces for unknown, suspicious, over-limit, spam,
  virus, or failed-auth inbound mail.
- Do not add MX records, activate receipt rules, or change inbound routability.
- Do not add IMAP/native/mobile clients, calendar/bookings/money workflows, or
  attachment viewing/composition.

Reference context:
- `MAIL-FOUNDATION-PLAN.md`
- `mail-foundation-spec.md`
- `MAIL-FOUNDATION-M4-PHASE.md`
- `MAIL-FOUNDATION-M5-PHASE.md`
- `MAIL-FOUNDATION-M6-PHASE.md`
- `docs/adr/0001-rust-lambda-and-react-spa.md`
- `docs/adr/0002-public-authenticated-text-only-ui.md`
- `docs/adr/0003-shared-cognito-strong-auth.md`
- `docs/adr/0004-project-owned-mail-infrastructure.md`
- `../ahara/INTEGRATION.md`
- M2 storage model in `db/migrations/001_create_mail_model.sql`
- Current API state/routes in `backend/api/src/lib.rs`
- Current external-service ports in `backend/shared/src/ports.rs`
- Current worker scaffolds in `backend/send-worker/` and
  `backend/feedback-handler/`
- Current M6 frontend API/auth/mailbox/routing-admin code in `frontend/src/`
- Current SES/SNS/Lambda Terraform in `infrastructure/terraform/`

Exit gate:
- Focused Rust tests for outbound compose/reply, send worker, forwarding, API
  routes, and feedback handling pass.
- Focused frontend tests for text-only compose/reply, queued status, forwarding
  admin, suppression/refusal errors, and no HTML rendering pass.
- `make ci`
- Source scans confirm no HTML rendering escape hatch, no outbound attachment
  composition, no synchronous SES send from API routes, and no MX/routability
  Terraform changes.

## Step 1 - Confirm forwarding granularity and outbound safety defaults  [DECISION]

Files:
- No code files. This is the M7 semantics decision before implementation.

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M7 has a phase-level `[DECISION]`: confirm
  forwarding rule granularity for MVP.
- `mail-foundation-spec.md` requires outbound rate limits and retry backoff,
  but does not define concrete limits.
- M2 schema already supports domain-level and address-level forwarding rules,
  suppressions, and outbound work rows.

Change:
- Stop for user confirmation before editing code.
- Confirm forwarding granularity:
  - Recommended M7 default: address-scoped forwarding only. A forwarding rule
    maps one configured accepted address to one target address. Domain-scoped
    rows remain in the database schema but are not exposed or processed in M7.
- Confirm outbound safety defaults:
  - Recommended M7 enqueue rate limit: at most 60 queued compose/reply/forward
    creations per normalized `from_address` per rolling hour.
  - Recommended M7 worker batch limit: claim at most 25 due outbound work rows
    per invocation.
  - Recommended retry policy: max 5 send attempts with backoff of 5 minutes,
    30 minutes, 2 hours, 8 hours, then permanent failure.

Verify:
- No automated verification. The executor records the confirmed values and
  then proceeds to step 2.

## Step 2 - Add shared outbound DTOs, validation, and MIME/header builders

Files:
- `backend/shared/src/outbound.rs`
- `backend/shared/src/lib.rs`
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`

Reference behavior:
- `mail-foundation-spec.md` says outbound compose is text-only, sends as a
  configured-domain address, generates `Message-ID` and `Date`, and sets
  `In-Reply-To` / `References` on replies.
- ADR-0002 says sender-controlled HTML is not rendered; M7 also must not send
  HTML bodies from the app.
- M5 already normalizes threading subjects and keeps RFC threading headers.

Change:
- Add shared outbound DTOs for:
  `ComposeMessageRequest`, `ReplyMessageRequest`,
  `OutboundMessageQueued`, `OutboundRecipient`, `OutboundMessageStatus`,
  `ForwardMessageRequest`, and outbound validation errors as needed.
- Add helpers for:
  - configured-domain `from_address` validation using existing route parsing,
  - recipient address normalization,
  - text/plain raw MIME construction,
  - generated `Message-ID` under the configured mail domain,
  - RFC-style `Date` header generation,
  - reply `In-Reply-To` and ordered `References` construction,
  - plaintext-only forwarded-message body generation with `Reply-To` set to
    the original sender.
- Add only the date/time dependency needed for stable Date header formatting.
- Do not add SQL, API routes, SES calls, or frontend code in this step.

Verify:
- Before the change:
  `! rg "ComposeMessageRequest|ReplyMessageRequest|build_outbound_mime|OutboundMessageQueued" backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared outbound_types`
- Red before: outbound DTO/builder symbols do not exist. Green after: tests
  cover text/plain MIME only, generated Message-ID/Date, reply
  `In-Reply-To`/`References`, `Reply-To` for generated forwards, address
  normalization, configured-domain sender refusal, and no HTML MIME part.

## Step 3 - Add outbound enqueue service and PostgreSQL repository  [depends on #1, #2]

Files:
- `backend/shared/src/outbound.rs`
- `backend/shared/tests/outbound_model.rs`

Reference behavior:
- `mail-foundation-spec.md` says outbound is enqueued and sent
  asynchronously; UI does not block on SES.
- M2 schema stores outbound messages in `messages` and send work in
  `outbound_work`; suppressions refuse outbound to suppressed addresses.
- `mail-foundation-spec.md` requires status reflected back, suppression checks
  before send, outbound rate limits, and transient retry backoff.

Change:
- Add an `OutboundService` trait with PostgreSQL and in-memory implementations:
  - `compose_message(request)`
  - `reply_to_message(source_message_id, request)`
  - `get_outbound_message(message_id)`
  - `claim_due_work(worker_id, limit)`
  - `mark_send_success(work_id, provider_message_id)`
  - `mark_send_retry(work_id, error)`
  - `mark_send_permanent_failure(work_id, error)`
- `compose_message` and `reply_to_message` insert:
  - `messages.direction = 'outbound'`
  - generated RFC message id and date,
  - text-only `body_text`,
  - sender/recipient rows,
  - `status = 'queued'`,
  - one `outbound_work` row with a stable idempotency key.
- `reply_to_message` derives thread, subject, `In-Reply-To`, and `References`
  from the source message and refuses replies to non-accepted inbound messages.
- Enforce the confirmed M7 enqueue rate limit before inserting work.
- Refuse suppressed recipients before inserting work.
- Store the SES provider id returned by the sender in `messages.ses_message_id`
  on success; keep inbound SES idempotency behavior unchanged.
- Add a PostgreSQL shape test using the existing container-network `psql`
  style when available; if this runner cannot execute the Postgres image, use
  the same explicit availability gate pattern as
  `backend/shared/tests/mailbox_model.rs`.

Verify:
- Before the change:
  `! rg "trait OutboundService|PgOutboundService|InMemoryOutboundService|claim_due_work" backend/shared/src backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared outbound`
- Red before: service/repository symbols do not exist. Green after: tests cover
  compose enqueue, reply threading headers, suppression refusal, configured
  sender refusal, rate-limit refusal, due-work claim locking, success status,
  retry backoff, permanent failure, and database row shape.

## Step 4 - Add authenticated compose and reply API routes  [depends on #3]

Files:
- `backend/api/src/lib.rs`
- `backend/api/Cargo.toml`

Reference behavior:
- ADR-0002 requires every non-health app route to be authenticated.
- `mail-foundation-spec.md` says compose/reply are authenticated UI actions
  and do not block on SES.
- M6 API state already wires service traits through `ApiState` and in-memory
  doubles for tests.

Change:
- Add `OutboundService` to `ApiState`.
- Wire `PgOutboundService` in `ApiState::from_env` and
  `InMemoryOutboundService` in `ApiState::for_tests`.
- Add authenticated routes:
  - `POST /outbound/messages/compose`
  - `POST /mailbox/messages/{message_id}/reply`
  - `GET /outbound/messages/{message_id}`
- Return stable JSON DTOs from the shared outbound module.
- Do not call SES directly from these routes.
- Do not add forwarding routes in this step.

Verify:
- Before the change:
  `! rg "/outbound/messages/compose|/mailbox/messages/\\{message_id\\}/reply|OutboundService" backend/api/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p api outbound_api`
- Red before: routes/service state do not exist. Green after: route tests cover
  auth required, compose returns queued status, reply returns queued threaded
  headers, suppressed-recipient refusal, invalid sender/recipient validation,
  non-accepted source reply refusal, status readback, and no synchronous
  `MailSender` call.

## Step 5 - Add SES raw-message sender implementation  [depends on #2]

Files:
- `backend/shared/src/ses_mail_sender.rs`
- `backend/shared/src/lib.rs`
- `backend/shared/src/ports.rs`
- `backend/Cargo.toml`
- `backend/shared/Cargo.toml`

Reference behavior:
- ADR-0004 keeps SES resources project-owned and IAM scoped to the configured
  SES identity.
- M3 Terraform already grants Lambda `ses:SendEmail` and `ses:SendRawEmail`
  on the project SES identity.
- M4 defined the `MailSender` port; M7 now provides the AWS implementation.

Change:
- Add an SES-backed `MailSender` implementation using the AWS SDK and raw MIME
  bytes from `OutboundMailRequest`.
- Preserve the `MailSender` trait boundary so shared outbound worker tests can
  keep using `InMemoryMailSender`.
- Map AWS provider message ids into `OutboundMailResponse.provider_message_id`.
- Map AWS SDK failures to public-safe `AppError::ExternalService` without raw
  message body/header leakage.
- Do not call this implementation from API routes.

Verify:
- Before the change:
  `! rg "SesMailSender|aws_sdk_ses|aws-sdk-ses|send_raw" backend/shared backend/Cargo.toml`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared ses_mail_sender`
- Red before: SES sender symbols/dependency do not exist. Green after: tests
  cover construction from config, safe error mapping, provider id mapping via a
  testable client seam, and no raw MIME leakage in public errors.

## Step 6 - Implement send-worker claim/send/retry loop  [depends on #3, #5]

Files:
- `backend/shared/src/outbound.rs`
- `backend/send-worker/src/lib.rs`
- `backend/send-worker/src/main.rs`
- `backend/send-worker/Cargo.toml`

Reference behavior:
- `mail-foundation-spec.md` requires asynchronous outbound send, transient SES
  retry with backoff, suppression checks, status updates, and outbound rate
  limiting.
- M4 send-worker is currently a stable no-op scaffold.
- M7 step 1 confirms worker batch and retry defaults.

Change:
- Add an `OutboundSendWorker` service that composes `OutboundService` and
  `MailSender`.
- Claim due `outbound_work` rows up to the confirmed worker batch limit.
- For each claimed row:
  - re-check suppressions before send,
  - send raw text/plain MIME through `MailSender`,
  - mark message/work `sent` with provider id on success,
  - mark retry with confirmed backoff on transient external-service failure,
  - mark permanent failure after the confirmed max attempt count.
- Update the Lambda handler to instantiate `PgOutboundService` and
  `SesMailSender` from environment/config.
- Return only counts/statuses from the handler; do not log raw message content.

Verify:
- Before the change:
  `! rg "OutboundSendWorker|claim_due_work|mark_send_success|SesMailSender" backend/send-worker backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared outbound_worker`
  and
  `cargo test --manifest-path backend/Cargo.toml -p send-worker`
- Red before: worker service symbols do not exist. Green after: tests cover
  batch limit, success status update, transient retry backoff, permanent
  failure, suppression refusal at send time, and handler response counts.

## Step 7 - Add forwarding rule service for confirmed MVP granularity  [depends on #1]

Files:
- `backend/shared/src/forwarding.rs`
- `backend/shared/src/lib.rs`
- `backend/shared/tests/forwarding_model.rs`

Reference behavior:
- M2 schema supports `forwarding_rules` with `rule_kind`, domain/address scope,
  target address, normalized target address, and active state.
- `mail-foundation-spec.md` requires resend-forward to a target address through
  the outbound path, not envelope forwarding/SRS.
- M7 step 1 confirms which rule granularity is active in MVP.

Change:
- Add `ForwardingRuleService` with PostgreSQL and in-memory implementations:
  - `list_rules()`
  - `upsert_rule(request)`
  - `deactivate_rule(rule_id)`
  - `active_rules_for_message(message_id)`
- Implement only the confirmed M7 granularity in service/API behavior.
- Validate target addresses with existing route parsing and normalize targets.
- Enforce active configured source domain/address requirements.
- Do not enqueue outbound messages in this step.

Verify:
- Before the change:
  `! rg "ForwardingRuleService|ForwardingRuleConfig|active_rules_for_message" backend/shared/src backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared forwarding_rules`
- Red before: forwarding service symbols do not exist. Green after: tests cover
  list/upsert/deactivate, target normalization, inactive source refusal,
  confirmed-granularity enforcement, active-rule lookup for accepted inbound
  messages, and database shape.

## Step 8 - Add authenticated forwarding-rule API routes  [depends on #7]

Files:
- `backend/api/src/lib.rs`
- `backend/api/Cargo.toml`

Reference behavior:
- ADR-0002 requires authenticated state-changing routes.
- M6 routing admin already uses authenticated domain/address config routes.
- M7 forwarding config must not expose MX/routability controls.

Change:
- Add `ForwardingRuleService` to `ApiState`.
- Wire `PgForwardingRuleService` in `ApiState::from_env` and
  `InMemoryForwardingRuleService` in `ApiState::for_tests`.
- Add authenticated routes:
  - `GET /forwarding/rules`
  - `POST /forwarding/rules`
  - `DELETE /forwarding/rules/{rule_id}`
- Return stable shared forwarding DTOs.
- Do not enqueue forwarding work from these admin routes.

Verify:
- Before the change:
  `! rg "/forwarding/rules|ForwardingRuleService" backend/api/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p api forwarding_api`
- Red before: forwarding API routes/state do not exist. Green after: tests
  cover auth required, list, create/upsert, deactivate, invalid target refusal,
  invalid source refusal, confirmed granularity enforcement, and no MX controls.

## Step 9 - Add forwarding planner over accepted inbound messages  [depends on #3, #7]

Files:
- `backend/shared/src/forwarding.rs`
- `backend/shared/src/outbound.rs`
- `backend/shared/src/inbound/service.rs`
- `backend/shared/tests/inbound_service.rs`
- `backend/shared/tests/forwarding_model.rs`

Reference behavior:
- `mail-foundation-spec.md` says resend-forward uses the same outbound path,
  sets `Reply-To` to the original sender, never resends quarantined/rejected
  mail, never resends mail that failed inbound SPF/DKIM/DMARC, and is
  idempotent against the source message.
- M5 `InboundIngestService` is the point where accepted inbound messages become
  durable.
- M2 `outbound_work.source_message_id` and unique `idempotency_key` support
  no-double-forwarding.

Change:
- Add a `ForwardingPlanner` that, for one accepted inbound source message:
  - loads active rules from `ForwardingRuleService`,
  - refuses source messages that are not accepted inbound `received`,
  - refuses source messages where SPF, DKIM, DMARC, or overall auth verdict is
    absent or not `pass`,
  - creates one outbound text-only forward message per active target through
    `OutboundService`,
  - sets `Reply-To` to the original sender,
  - uses an idempotency key shaped from source message id and rule id.
- Wire the planner into inbound ingest after successful accepted-message
  persistence. Rejected/quarantined/unknown-recipient paths must not call it.
- Keep forwarding failures from causing duplicate inbound persistence; surface
  safe failure counts/errors only.

Verify:
- Before the change:
  `! rg "ForwardingPlanner|enqueue_forward|source_message_id.*forward" backend/shared/src backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared forwarding`
  and
  `cargo test --manifest-path backend/Cargo.toml -p shared inbound_service`
- Red before: forwarding planner/enqueue symbols do not exist. Green after:
  tests cover accepted/pass-auth forwarding, no double-forwarding on retry,
  `Reply-To` original sender, same outbound path/work row, auth-failed refusal,
  quarantined/rejected refusal, suppression refusal, and no app-side bounce.

## Step 10 - Add SES feedback parser and repository updater  [depends on #3]

Files:
- `backend/shared/src/feedback.rs`
- `backend/shared/src/lib.rs`
- `backend/shared/tests/feedback_model.rs`

Reference behavior:
- `mail-foundation-spec.md` says SES bounce/complaint SNS updates
  suppression and originating message status.
- M2 schema stores suppressions and outbound message statuses `bounced` and
  `complained`.
- M3 Terraform wires SES identity bounce/complaint notification topics to the
  feedback handler and excludes original headers.

Change:
- Add typed parsing for SNS-wrapped SES bounce and complaint notifications.
- Extract provider SES message id, feedback type, and affected recipient
  addresses.
- Add `FeedbackService` with PostgreSQL and in-memory implementations:
  - `process_feedback(event)`
  - upsert suppression rows with reason `bounce` or `complaint`,
  - update originating outbound message status by matching
    `messages.ses_message_id`,
  - update related `outbound_work` status when present.
- Preserve public-safe errors and logs; do not store or log original message
  headers.

Verify:
- Before the change:
  `! rg "FeedbackService|SesFeedbackEvent|process_feedback|complained" backend/shared/src backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared feedback`
- Red before: feedback service/parser symbols do not exist. Green after: tests
  cover bounce parsing, complaint parsing, multiple recipients, suppression
  upsert idempotency, originating message status updates, unknown provider id
  safe handling, and database shape.

## Step 11 - Implement feedback-handler Lambda behavior  [depends on #10]

Files:
- `backend/feedback-handler/src/lib.rs`
- `backend/feedback-handler/src/main.rs`
- `backend/feedback-handler/Cargo.toml`

Reference behavior:
- M4 feedback-handler is currently a stable no-op scaffold.
- M3 Terraform subscribes the feedback handler Lambda to SES bounce and
  complaint SNS topics.
- `mail-foundation-spec.md` requires bounce/complaint suppression and status
  updates.

Change:
- Update the handler to parse SNS event payloads and call `FeedbackService`.
- Instantiate `PgFeedbackService` from environment/config.
- Return only safe counts/statuses: processed records, suppressed recipients,
  and updated messages.
- Treat malformed records as validation errors with no raw payload/header
  leakage.

Verify:
- Before the change:
  `! rg "process_feedback|SesFeedbackEvent|suppressed_recipients" backend/feedback-handler/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p feedback-handler`
- Red before: handler feedback behavior symbols do not exist. Green after:
  tests cover bounce event handling, complaint event handling, malformed SNS
  refusal, idempotent duplicate event handling through an in-memory service,
  and safe response payloads.

## Step 12 - Add frontend outbound and forwarding API methods  [depends on #4, #8]

Files:
- `frontend/src/types.ts`
- `frontend/src/api.ts`
- `frontend/src/api.test.ts`

Reference behavior:
- M7 backend routes return shared outbound and forwarding DTOs.
- ADR-0002 requires authenticated API calls for application behavior.
- `mail-foundation-spec.md` keeps compose text-only and outbound attachments
  deferred.

Change:
- Add TypeScript DTOs matching M7 API JSON:
  outbound compose/reply requests, queued outbound message/status, forwarding
  rule requests/responses.
- Add API methods:
  - `composeMessage`
  - `replyToMessage`
  - `fetchOutboundMessage`
  - `listForwardingRules`
  - `upsertForwardingRule`
  - `deactivateForwardingRule`
- Preserve bearer-token injection and typed error normalization.
- Do not add attachment upload, HTML body, or raw MIME methods.

Verify:
- Before the change:
  `! rg "composeMessage|replyToMessage|ForwardingRule|fetchOutboundMessage" frontend/src`
- After the change:
  `cd frontend && pnpm exec vitest run src/api.test.ts`
- Red before: frontend API symbols do not exist. Green after: tests cover
  compose, reply, status, forwarding list/upsert/deactivate, bearer header
  injection, error normalization, and absence of attachment/raw-MIME methods.

## Step 13 - Add text-only compose and reply UI  [depends on #4, #12]

Files:
- `frontend/src/mailbox.tsx`
- `frontend/src/mailbox.test.tsx`
- `frontend/src/App.tsx`
- `frontend/src/index.css`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M7 requires compose/reply API and UI.
- `mail-foundation-spec.md` says MVP compose is text-only, outbound is queued
  asynchronously, and reply threading headers are generated by the backend.
- ADR-0002 keeps all app behavior behind auth and avoids HTML rendering.

Change:
- Add a compact compose panel/action reachable from the authenticated mailbox
  shell.
- Add reply action from message detail.
- Fields: from configured address, to/cc/bcc as plain text address inputs,
  subject, and body text.
- Submit compose/reply through the API client and show queued status returned
  by the API.
- Do not add rich text, HTML preview, attachment controls, or synchronous send.
- Ensure text fits and the operational UI remains dense; use icons for obvious
  actions.

Verify:
- Before the change:
  `! rg "ComposeMessage|Reply to message|compose-message|replyToMessage" frontend/src`
- After the change:
  `cd frontend && pnpm exec vitest run src/mailbox.test.tsx src/App.test.tsx`
- Red before: compose/reply UI symbols do not exist. Green after: tests cover
  auth-gated compose, text-only submit, queued status display, reply action
  from detail, suppression/validation API error display, no attachment
  controls, and no HTML rendering.

## Step 14 - Add forwarding admin UI  [depends on #8, #12]

Files:
- `frontend/src/routingAdmin.tsx`
- `frontend/src/routingAdmin.test.tsx`
- `frontend/src/App.tsx`
- `frontend/src/index.css`

Reference behavior:
- M6 routing admin already exposes domain/address routing policy controls.
- M7 adds forwarding-rule management, but not MX/routability controls.
- M7 step 1 confirms the active forwarding granularity.

Change:
- Extend routing admin with a forwarding-rule section.
- Show active forwarding rules, source address/domain according to confirmed
  granularity, target address, and active state.
- Add controls to create/upsert and deactivate forwarding rules.
- Validate with API errors; do not infer Gmail targets from display names.
- Do not add MX, receipt-rule, or routability controls.

Verify:
- Before the change:
  `! rg "Forwarding rules|upsertForwardingRule|deactivateForwardingRule" frontend/src`
- After the change:
  `cd frontend && pnpm exec vitest run src/routingAdmin.test.tsx src/App.test.tsx`
- Red before: forwarding admin UI does not exist. Green after: tests cover
  rule list rendering, create/upsert, deactivate, invalid target API error,
  confirmed-granularity fields only, and no MX/routability controls.

## Step 15 - Run the M7 exit gate  [depends on #14]

Files:
- No implementation files unless an earlier verification exposes a build-only
  wiring miss directly caused by M7.

Reference behavior:
- M7 exit in `MAIL-FOUNDATION-PLAN.md` requires `make ci` green and tests for
  reply threading headers, suppression refusal, retry backoff,
  no double-forwarding, auth-failed forward refusal, and bounce/complaint
  suppression.
- M7 scope guard forbids outbound attachments, HTML rendering/sending,
  synchronous API sends, direct forwarding outside the outbound path, and
  MX/routability changes.

Change:
- Run the focused M7 tests first, then the repository-wide gate.
- If a focused test fails, fix only the M7-scoped cause in the files named by
  the failed step.
- Do not continue into M8.

Verify:
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p shared outbound`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p shared forwarding`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p shared feedback`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p api outbound_api forwarding_api`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p send-worker`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p feedback-handler`
- Run:
  `cd frontend && pnpm exec vitest run`
- Run:
  `make ci`
- Run:
  `! rg -n "dangerouslySetInnerHTML|innerHTML|outerHTML|text/html|Content-Type: text/html" frontend/src backend/api/src backend/shared/src backend/send-worker/src`
- Run:
  `! rg -n "attachment|Attachment|multipart/mixed|Content-Disposition: attachment" frontend/src backend/shared/src/outbound.rs backend/api/src`
- Run:
  `! rg -n "MailSender|send_mail\\(|SesMailSender" backend/api/src`
- Run:
  `! rg -n "type\\s*=\\s*\\\"MX\\\"|MX[[:space:]]+|^\\s*enabled\\s*=\\s*true|^\\s*active\\s*=\\s*true" infrastructure/terraform -g '*.tf'`
- The executor reports the actual output for the focused tests and `make ci`,
  explains any broad-scan false positives with file/line references, then
  stops at the end of M7.
