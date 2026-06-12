# Mail Foundation Spec

Self-hosted mail transport + text-only web client on AWS, replacing the Workspace account, built to grow into the mentoring portal (CRM, bookings, money). Storage on the shared Postgres instance. Name TBD.

Decisions below are settled from design discussion. Tech stack is fixed to the primitives named here (SES inbound/outbound, S3, SNS, Lambda, Postgres); mechanisms marked "per infra" are deliberately left to the target ecosystem. No tech beyond these is mandated.

---

## Scope

**MVP, in scope**
- Receive mail for configured domains via SES inbound.
- Strip to text at ingest; store parsed metadata + S3 pointer in Postgres, raw MIME in S3.
- Web UI as the primary read/compose surface (text-only).
- Send mail as configured-domain addresses via SES.
- Resend-forward selected inbound mail to a target address (personal Gmail).
- Bounce/complaint handling with suppression.
- Per-domain routing policy (allowlist or catch-all).
- Single-user, authenticated public HTTPS surface behind shared Cognito; no unauthenticated app surface.
- Thin `contact` table as the CRM seam, with messages linkable to contacts.

**Deferred / out of scope for MVP**
- Attachment extraction or in-app attachment handling (raw MIME retained as the escape hatch).
- Outbound attachment composition.
- Calendar / ICS, bookings, money tracking (data model must not preclude).
- Mentee-facing logins / any client-facing surface (see Open Decisions).
- IMAP / mobile native clients.

---

## Pipeline shape

MX → SES inbound (configured domains) → receipt rule writes raw MIME to S3 + triggers parse (Lambda) → parse extracts headers/threading keys/recipients/auth results/spam and virus verdicts/text body/attachment metadata, applies per-domain policy, security disposition, and ingest caps, writes accepted, quarantined, or rejected audit records to Postgres via pooled connection. Reads/compose happen in the web UI against Postgres. Outbound (compose, resend) goes through an outbound path → SES, with status reflected back. SES bounce/complaint SNS → handler → suppression + message status.

---

## Domains & Routing

- Each configured domain carries a routing policy: `allowlist` or `catchall`. Policy is per-domain, independent across domains.
- `allowlist`: only enumerated local parts are accepted; non-matching recipients are rejected/dropped at ingest per policy.
- `catchall`: all local parts at the domain are accepted. Allowed where intended (e.g. signup-only throwaway domains); not the default.
- `+tag` convention: parse and strip the `+suffix` from the local part, match the base address, retain the full tag on the stored message for filtering/disposable-address tracking.
- Per-domain DKIM/verification state tracked for operational visibility.

---

## Ingest (receiving)

- SES receipt rule stores raw MIME to S3 (streamed, not buffered) and triggers parse.
- Parse extracts: `From` (address + display), recipients (`To`/`Cc`), `Subject`, `Date`, `Message-ID`, `In-Reply-To`, `References`, the matched configured address/domain, SES-provided SPF/DKIM/DMARC/spam/virus verdicts, body text, and attachment metadata only.
- Security disposition is enforced before a message can enter the normal inbox: spam `pass` + virus `pass` is accepted; spam `fail`, spam `gray`, or spam `processing_failed` is quarantined; virus `gray` or `processing_failed` is quarantined; virus `fail` is rejected/dropped with only minimal audit metadata. Missing scan verdicts are treated as indeterminate and quarantined. Quarantined/rejected messages are never resent-forwarded and never appear in normal mailbox queries.
- Body text: use `text/plain` when usable; otherwise convert HTML → readable plaintext. Conversion strips all markup/scripts/styles/remote references, preserves link URLs inline as text, preserves paragraph/list structure. HTML is never rendered or stored for rendering. Conversion is not a security boundary — worst case is ugly text.
- Attachments: not extracted into the app. Record metadata refs only (declared filename, declared content-type, size, count). Treat declared filenames as untrusted. Original bytes remain retrievable from raw MIME.
- Apply per-domain routing policy before normal persistence (allowlist mismatch → reject/drop per policy). Do not generate app-side bounces for suspicious or unknown inbound mail; avoid backscatter.
- Ingest caps (reject past limit): max message size, max multipart nesting depth, max attachment count.
- Idempotency: keyed on SES message id / S3 object key so SES/Lambda retries do not duplicate rows.
- Parse Lambda runs under a concurrency cap.

---

## Storage / Data Model

Entity shape and relationships, not DDL. Recipient storage and outbound-queue placement are flagged in Open Decisions.

**domain**
- domain name, routing policy (`allowlist` | `catchall`), active flag, DKIM/verification state.

**address** (allowlist entries)
- domain (FK), accepted local part. Base address that `+tag` parsing resolves to.

**message** (core; inbound + outbound)
- id; direction (`inbound` | `outbound`).
- RFC `Message-ID`, `In-Reply-To`, `References`; thread (FK).
- from address, from display; subject; date.
- matched configured address/domain (inbound).
- body text; s3_raw_key (pointer to raw MIME).
- auth results: SPF/DKIM/DMARC pass-fail + overall verdict (inbound).
- spam and virus scan results plus security disposition (`accepted` | `quarantined` | `rejected`) and reason.
- contact (FK, nullable).
- read/unread.
- status (inbound: `received` | `quarantined` | `rejected`; outbound: `queued` | `sending` | `sent` | `failed` | `bounced` | `complained`); send-attempt count; next-retry; last error.
- has_attachments flag; attachment count.
- received_at / sent_at; size.

**recipient** (or arrays on message — Open Decision)
- message (FK), type (`to` | `cc` | `bcc`), address.

**attachment_ref**
- message (FK), declared filename (untrusted; sanitize on any display/download), declared content-type, size. Bytes live only in the message's raw MIME.

**thread**
- id, normalized subject, participants, last activity, message count. Derived from threading headers (`Message-ID` / `In-Reply-To` / `References`).

**suppression**
- address, reason (`hard_bounce` | `complaint`), source message (FK), created_at. Outbound to suppressed addresses is refused.

**contact** (CRM seam, thin for MVP)
- id, name, primary address(es), notes, created_at. `message.contact_id` → contact.

**auth store**
- Shared Cognito identity for MVP, structured to allow more later; MFA/passkey posture and app-session metadata per infra.

**Isolation**
- This app owns a dedicated Postgres role + schema on the shared instance. Grants exclude any (future) financial/CRM-sensitive schemas. All access via parameterized queries.

---

## Reading / Web UI

- Primary surface. Text-only rendering throughout; no HTML rendering anywhere.
- Conversation/threaded view backed by the thread model.
- Links rendered as inert text by default. If made clickable, restrict to `http`/`https`/`mailto`; strip `data:`/`javascript:`.
- Surface SPF/DKIM/DMARC verdict per message. Show the real `From` address prominently; treat display name as untrusted.
- Attachments shown as metadata with a "download original" action sourced from raw MIME only for accepted messages with clean virus verdicts (sanitize filename on download).
- Read/unread, search (scope per Open Decisions), per-contact association view.

---

## Compose / Sending

- Send as a configured-domain address via SES. Triggered only by an authenticated UI action or fixed-template system code.
- Set headers correctly: `From` = configured address, generated `Message-ID`, `Date`; on replies set `In-Reply-To` and `References` so recipients' clients thread.
- Outbound is enqueued and sent asynchronously; UI does not block on SES. Status reflected back to the message. Retry transient SES failures with backoff.
- Check suppression before send; refuse suppressed recipients.
- Outbound rate limit.
- MVP composes text only (outbound attachments deferred).

---

## Resend-Forward

- Forward rules route selected inbound mail to a target (personal Gmail). Granularity (per-address / per-domain / conditional rule) per Open Decisions.
- Resend as a **new** message from a configured-domain address — not envelope forwarding, no SRS. Set `Reply-To` to the original sender so replies route correctly. Original sender surfaced in the message context.
- Forwarded body is the stripped text version (accepted consequence; the original remains in S3).
- Never resend quarantined or rejected inbound mail.
- **Never resend mail that failed inbound SPF/DKIM/DMARC.** Reputation protection is hard.
- Resend uses the same outbound path: suppression check, rate limit, status, retry.
- Idempotent against the source message so reprocessing never double-forwards.

---

## Bounce / Complaint Handling

- SES bounce and complaint SNS topics feed a handler.
- Hard bounce or complaint → add address to suppression and update the originating message status.
- Track bounce/complaint rates; alarm on thresholds (SES reputation is enforcement-relevant).

---

## Security Requirements

Imperative, traceable to the threat discussion; not re-justified here.

- Web UI exposed only through the platform HTTPS path (CloudFront/shared ALB) and shared Cognito; every app route requires authentication.
- Strong auth: passkey/WebAuthn or TOTP MFA. Short-lived sessions. `SameSite` cookies + CSRF protection on all state-changing actions.
- No HTML rendering on any surface (eliminates the email-HTML XSS class and reduces the risk of a public authenticated UI). Links inert-by-default with scheme restriction as above.
- `From` is display only, never identity. Surface auth verdicts. Do not auto-associate an unauthenticated sender to a contact without marking it unverified.
- Least-privilege IAM scoped to the specific SES identities, S3 prefixes, and SNS topics used. No broad grants.
- Secrets in a parameter/secret store; prefer role-based AWS credentials over static long-lived keys.
- Dedicated Postgres role + schema; grants exclude financial/CRM-sensitive schemas; parameterized queries only.
- Per-object authorization scoped to owner; opaque IDs. (Becomes load-bearing if a client-facing surface is added.)
- S3: block public access, SSE at rest, TLS in transit.
- SES spam/virus verdicts are enforcement inputs, not passive metadata: only clean accepted messages are eligible for normal mailbox display, resend-forwarding, or original download.
- Do not log full headers or bodies; scrub PII from logs.
- Never use untrusted attachment filenames as storage keys; sanitize on display/download.
- Do not route the most sensitive external accounts' password resets to these domains.

---

## Operational / Cost Controls

- CloudWatch billing alarm.
- Lambda concurrency caps on ingest and any send worker to bound cost and blast radius under flood.
- Connection pooling in front of the shared Postgres so floods don't starve the business app (pgbouncer discussed; per infra).
- Ingest caps as above (size, nesting depth, attachment count).
- Prefer allowlist over catch-all per domain to limit dictionary-attack volume.
- Raw MIME S3 lifecycle/retention policy — per infra.

---

## Open Decisions

1. **Mentee-facing logins vs pure private CRM.** Determines whether a client-facing surface exists as a separately-authz'd module and whether multi-tenant authz / IDOR becomes a primary concern. MVP is scoped single-user/private; the data model should not preclude either path.
2. **Outbound attachment composition** — if/when.
3. **Forward target flexibility** — Gmail-only vs arbitrary per-rule targets.
4. **Forward granularity** — per-address, per-domain, or conditional rules.
5. **Recipient storage** — join table vs arrays on `message`.
6. **Outbound queue** — folded into `message` (status fields) vs dedicated table.
7. **Search requirements** for MVP — full-text over body/subject, scope, ranking.
8. **Raw MIME retention** policy.
9. **System name / brand.**
