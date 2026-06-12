# Mail Foundation - Implementation Plan

Build `ahara-business` from a spec-only repository into a deployable mail foundation: SES inbound/outbound, raw MIME in S3, parsed text and metadata in shared PostgreSQL, bounce/complaint handling, and a public authenticated text-only web UI. Attachment handling, calendar/bookings/money, mentee-facing accounts, IMAP, and native mobile clients stay outside the MVP architecture and are tracked in the backlog.

## Confirmed decisions

- Runtime stack: Rust Lambda workspace plus TypeScript/React SPA.
- UI boundary: public CloudFront/shared ALB path with shared Cognito auth.
- Mail-content safety: ingest strips HTML/CSS/script/remote references to plaintext, and the UI renders stored plaintext only.
- Strong auth: shared Cognito gains required TOTP or passkey/WebAuthn support.
- Mail infrastructure ownership: project Terraform owns SES/S3/SNS/Lambda resources, with new `ahara-infra` deployer policy primitives for SES and private mail S3 storage.

## Context / reuse map

| Area | Reuse | Source |
| ---- | ---- | ---- |
| Platform rules | Shared ALB, VPC, RDS, Cognito, state bucket, tag/SSM discovery | `../ahara/INTEGRATION.md` |
| Website hosting | `ahara-tf-patterns/modules/website` | `../ahara-tf-patterns/modules/website/` |
| HTTP API | `ahara-tf-patterns/modules/alb-api` with `jwt-validation` | `../ahara-tf-patterns/modules/alb-api/` |
| Async Lambdas | `ahara-tf-patterns/modules/lambda` | `../ahara-tf-patterns/modules/lambda/` |
| Database migrations | Platform `db-migrate` flow and `platform.yml` | `../ahara/INTEGRATION.md` |
| Existing platform gaps | SES deployer permissions and private mail-storage S3 policy primitive | `../ahara-infra/infrastructure/terraform/control/modules/managed-project/` |
| Product requirements | Mail foundation spec | `mail-foundation-spec.md` |
| Durable decisions | ADRs 0001-0004 | `docs/adr/README.md` |

## Cross-cutting constraints

- Every route and state-changing action assumes a public authenticated app surface; auth, CSRF, session lifetime, and rate limiting are load-bearing.
- Sender-controlled HTML is never rendered. HTML conversion is a usability transform and an XSS-risk reduction, not the only security boundary.
- IAM grants stay scoped to project SES identities, S3 prefixes/buckets, SNS topics, and Lambda names.
- Shared PostgreSQL access uses a project database/role and migrations only; migrations do not create users, roles, grants, or databases.
- Lambdas keep business logic in library crates so parsing, policy, SQL, retry, and suppression behavior can be tested outside Lambda glue.
- Raw MIME remains private S3 data. Logs omit message bodies and full headers.
- Placeholder homes stay README-only until a phase registers buildable packages.

## Milestones

### M0 - Project Scaffold And Platform Registration
Establish runnable project structure and deployment eligibility.

- Add buildable Rust workspace, frontend package, Terraform root, deploy script, and shared CI workflow.
- Register `ahara-business` deployer role in `ahara-infra` with website, alb-api, cognito-app, lambda, terraform-state, db-migrate, sns, SES, and private S3 storage permissions.
- Register the project database in `ahara-infra` migration projects with a PostgreSQL-safe database name.
- Expand `make ci` from docs-only validation to lint, format-check, typecheck, test, and Terraform format checks.
- Exit: `make ci` green; deployer and database registration changes are present in `ahara-infra`; no placeholder workspace members remain.

### M1 - Shared Cognito Strong Auth
Make the public authenticated surface match the mail security posture.

- Extend shared Cognito Terraform for required TOTP or passkey/WebAuthn support.
- Keep project app-client creation in `ahara-business` Terraform through the platform `cognito-app` module.
- Verify user-access gating remains compatible with the pre-auth Lambda and app client.
- Exit: `make ci` green in affected repos; Cognito strong-auth Terraform plans cleanly; app client can still obtain tokens for ALB JWT validation.

### M2 - Database Model And Migrations
Create the durable mail data model on shared PostgreSQL.

- Add migrations for domains, accepted addresses, messages, recipients, attachment refs, threads, contacts, forwarding rules, suppressions, and outbound work.
- Add rollback files and idempotent seed data for initial domain/address routing.
- Add Rust storage tests with real PostgreSQL through testcontainers.
- **[DECISION]** Confirm normalized `recipient` rows as the storage shape.
- **[DECISION]** Confirm a dedicated outbound work table instead of folding all queue state into `message`.
- Exit: `make ci` green; migrations apply and roll back locally; idempotent seeds can run twice.

### M3 - Mail AWS Infrastructure
Own the SES/S3/SNS primitives needed for mail transport.

- Add project Terraform for SES domain identities, DKIM records, MX records, receipt rule set/rules, raw MIME S3 bucket, lifecycle settings, SNS feedback topics, and Lambda permissions.
- Wire receipt rules to S3 storage plus ingest Lambda invocation.
- Wire SES bounce/complaint events to SNS and the feedback handler.
- Add CloudWatch alarms for SES reputation and cost/blast-radius controls.
- **[DECISION]** Choose raw MIME lifecycle retention defaults.
- Exit: `make ci` green; Terraform plan shows scoped SES/S3/SNS/Lambda resources only; no broad deployer grants are required.

### M4 - Backend Foundations
Build the API and worker skeletons around shared logic.

- Add Rust crates for API, ingest, send worker, feedback handler, and shared libraries.
- Add shared config, database pool setup, error types, auth context extraction, and external-service traits for SES/S3/SNS.
- Add API health, authenticated user context, domain/address config routes, and contact basics.
- Exit: `make ci` green; unit tests cover config, auth extraction, policy parsing, and in-memory service doubles.

### M5 - Inbound Ingest Pipeline
Persist safe text mail from SES events.

- Parse SES event metadata, fetch raw MIME from S3, enforce size/nesting/attachment caps, and make ingest idempotent by SES message ID/S3 key.
- Extract headers, auth verdicts, SES spam/virus verdicts, recipients, threading keys, usable text/plain, HTML-to-text fallback, attachment metadata, and plus-tags.
- Enforce spam/virus disposition before normal mailbox persistence: clean mail is accepted, spam/indeterminate scans are quarantined, virus failures are rejected/dropped with minimal audit metadata, and non-accepted mail is never resent-forwarded.
- Apply per-domain allowlist/catchall policy before persistence.
- Build thread derivation and contact-linking primitives without trusting display names as identity.
- Exit: `make ci` green; parser tests cover MIME variants, HTML stripping, dangerous links, spam/virus disposition, retry idempotency, routing policy, and ingest caps.

### M6 - Text-Only Mail UI And Read API
Deliver the authenticated mailbox reading workflow.

- Add React SPA auth bootstrap, runtime config, API wrapper, mailbox list, thread view, message detail, contact association, routing-policy admin, read/unread, and search.
- Render plaintext bodies only, show real sender address, auth verdicts, and security disposition, treat links as inert text by default, exclude quarantined/rejected mail from normal mailbox lists, and sanitize attachment metadata display.
- Add API routes for mailbox queries, thread detail, contact links, message state, and search.
- **[DECISION]** Choose the MVP product/system name used in UI and DNS.
- Exit: `make ci` green; UI tests cover authenticated loading states, text-only rendering, inert dangerous links, and auth-verdict display.

### M7 - Compose, Sending, Forwarding, And Feedback
Complete outbound mail workflows.

- Add compose/reply API and UI with generated Message-ID, Date, In-Reply-To, and References headers.
- Add async send worker retries, suppression checks, outbound rate limits, and SES status updates.
- Add resend-forward rules to Gmail through the same outbound path, with `Reply-To` set to the original sender and failed-auth inbound messages refused.
- Add bounce/complaint SNS handler updates for suppression and originating message status.
- **[DECISION]** Confirm forwarding rule granularity for MVP.
- Exit: `make ci` green; tests cover reply threading headers, suppression refusal, retry backoff, no double-forwarding, auth-failed forward refusal, and bounce/complaint suppression.

### M8 - Operational Hardening And Release Readiness
Close the production-readiness loop.

- Add PII-safe logging, structured metrics, alarms, Lambda concurrency caps, S3 public-access blocks/encryption, and least-privilege IAM review.
- Add deploy documentation, `.env.example`, smoke-check procedure, and CI/deploy workflow alignment with `../ahara/CI-WORKFLOW.md`.
- Run platform deploy dry checks and document first-domain DNS verification steps.
- Exit: `make ci` green; Terraform plan is reviewed; smoke procedure verifies receive, read, send, forward, and bounce/complaint handling in a controlled domain.

### Decisions needing your input

| Where | Decision you own |
| ---- | ---- |
| M2 | Confirm normalized `recipient` rows as the storage shape |
| M2 | Confirm a dedicated outbound work table instead of folded queue state |
| M3 | Choose raw MIME lifecycle retention defaults |
| M6 | Choose the MVP product/system name used in UI and DNS |
| M7 | Confirm forwarding rule granularity for MVP |
