# Mail Foundation Spec

Ahara Mail is the SES-backed mail transport and text-only web client for Ahara
business correspondence. It runs on the shared Ahara AWS platform, stores mail
metadata and searchable text in shared PostgreSQL, keeps raw MIME in private S3,
and exposes an authenticated web surface through shared Cognito.

Architecture decisions are recorded in [docs/adr/README.md](docs/adr/README.md).
Expansion work is tracked in [docs/backlog.md](docs/backlog.md).
The post-MVP Business Hub direction is described in
[docs/business-hub.md](docs/business-hub.md).

## Scope

- Receive mail for configured domains through SES inbound.
- Store raw MIME in private S3 and parsed metadata/text in PostgreSQL.
- Render mailbox content as plaintext in the authenticated web UI.
- Send text-only mail as configured-domain addresses through SES.
- Queue compose, reply, and forwarding work for asynchronous delivery.
- Resend-forward accepted pass-auth inbound mail through the outbound path.
- Process SES bounce and complaint feedback into message status and suppression
  records.
- Manage domains, accepted addresses, contacts, read state, search, and
  address-scoped forwarding rules from the web UI.
- Use shared Cognito with TOTP setup and Cognito access-token validation.
- Keep a thin contact model as the CRM boundary for later business workflows.

## Post-MVP Expansion Boundary

Ahara Mail remains the communication backbone for business correspondence.
Business Hub expansion happens in this repo and builds contact-linked workflows
around mail: calendar, booking, money tracking, mentee access, and
limited-audience file sharing.

Cross-app identity, object grants, and secure file-sharing permissions belong
to `ahara-access` rather than to Ahara Mail. Ahara Mail can link messages,
contacts, bookings, and business records to shared access resources, but it
does not own global external-user authorization.

## Pipeline Shape

Inbound flow:

1. Route 53 MX points `ahara.io` to SES inbound in `us-east-1`.
2. SES invokes the receipt gate synchronously.
3. The receipt gate accepts only configured recipients and applies rolling count
   limits before raw storage.
4. SES writes accepted raw MIME to the project S3 bucket under `raw/`.
5. SES invokes ingest asynchronously with the S3 pointer.
6. Ingest applies raw object limits, parses MIME, converts body text, enforces
   spam/virus disposition, updates threads, persists mailbox rows, and enqueues
   forwarding work.

Outbound flow:

1. Authenticated compose, reply, or forwarding actions create outbound message
   and work rows.
2. The scheduled send worker claims queued work, checks suppressions and rate
   limits, constructs text/plain MIME, and sends through SES.
3. SES bounce and complaint SNS notifications update outbound status and
   suppressions through the feedback handler.

## Domains And Routing

- Each domain has a routing policy of `allowlist` or `catchall`.
- `allowlist` accepts only active local parts in the `addresses` table.
- `catchall` accepts all local parts for an active domain.
- Plus-address labels resolve to the base local part and are retained on the
  stored message as `plus_tag`.
- The deployed seed configures `ahara.io` with active `chris` and `contact`
  local parts.

## Ingest

- SES receipt metadata supplies SPF, DKIM, DMARC, spam, and virus verdicts.
- Security disposition is enforced before normal mailbox persistence:
  - clean spam and virus verdicts are accepted;
  - spam failures, gray verdicts, processing failures, and missing scan values
    are quarantined;
  - virus failures are rejected with minimal audit metadata.
- Normal mailbox queries return accepted messages only.
- Forwarding processes accepted messages only when SPF, DKIM, and DMARC pass.
- MIME parsing extracts sender, recipients, subject, date, message identifiers,
  threading keys, matched route, body text, and attachment metadata.
- Body selection prefers usable `text/plain`; HTML fallback is converted to
  readable plaintext with scripts, styles, markup, remote references, and active
  links removed from the render path.
- Attachment bytes stay in raw MIME. The app stores declared filename,
  content-type, size, count, and optional content-id metadata.
- Ingest rejects messages past configured size, depth, count, or recent raw-byte
  caps and stores minimal rejected audit rows.
- SES message ID and S3 raw key provide idempotency for retry handling.

## Storage Model

The database schema stores:

- `domains` and `addresses` for routing and verification state.
- `contacts` for display names, primary addresses, and notes.
- `threads` for normalized subject, participants, last activity, and message
  count.
- `messages` for inbound/outbound state, body text, route match, security
  verdicts, contact links, read state, status, retry fields, attachment summary,
  and raw S3 pointer.
- `recipients` as normalized rows for `to`, `cc`, and `bcc`.
- `attachment_refs` as metadata pointers without app-owned attachment bytes.
- `forwarding_rules` as active address-scoped forwarding configuration.
- `suppressions` for bounce, complaint, and manual suppression.
- `outbound_work` as the dedicated queue and retry ledger.

The app uses the platform-managed `ahara-business` database credentials and
parameterized SQL through `sqlx`.

## Web UI

- The web UI is the primary read and compose surface.
- Auth states cover loading, sign-in, TOTP challenge, TOTP setup QR code, error,
  and signed-in app navigation.
- The signed-in workspace includes mailbox, sent mail, contacts, routing, and
  forwarding controls.
- Mailbox reads show real sender address, display name, recipients, auth
  verdicts, security disposition, body text, contact link, read state, threads,
  search results, and attachment metadata.
- Compose and reply send text-only outbound requests; reply preserves threading
  headers.
- Links in message bodies render as inert text.
- Sender display names, contact associations, and attachment filenames are
  display data rather than identity.

## Security Requirements

- Expose the app only through the platform HTTPS path and shared Cognito.
- Validate Cognito tokens at the ALB and again in the API before serving app
  data.
- Keep `/health` as the only unauthenticated API route.
- Render stored plaintext only; never render sender-provided HTML.
- Treat `From` as display metadata and surface auth verdicts beside sender
  information.
- Keep IAM scoped to project SES identities, S3 buckets/prefixes, SNS topics,
  and Lambda names.
- Keep secrets in platform parameter or secret stores and prefer role-based AWS
  credentials.
- Use the project PostgreSQL role and schema boundary with parameterized
  queries.
- Block public S3 access and encrypt stored raw MIME.
- Use SES spam and virus verdicts as enforcement inputs for mailbox visibility,
  forwarding, and original-content access.
- Omit message bodies, full headers, and raw email addresses from logs and
  operational metrics.
- Sanitize untrusted attachment filenames on display or download.
- Route sensitive external account recovery mail through domains with a mature
  operational posture.

## Operational Controls

- The receipt gate enforces 120 accepted messages per recipient per hour and
  240 accepted-recipient messages per hour across the configured domain.
- Ingest enforces a 10 MiB raw object cap, 25 MiB MIME cap, 50 MiB recent raw
  byte cap per hour, 20-part MIME depth cap, and 25-attachment metadata cap.
- Lambda reserved concurrency bounds API, receipt gate, ingest, send worker, and
  feedback handler execution.
- EventBridge invokes the send worker once per minute.
- Raw MIME lifecycle keeps current objects for 365 days, noncurrent versions for
  30 days, and aborts incomplete multipart uploads after one day.
- CloudWatch alarms cover SES reputation, Lambda errors/throttles/volume,
  outbound failures, inbound failures, flood-control rejections, and complaint
  feedback.
