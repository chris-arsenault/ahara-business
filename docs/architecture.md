# Architecture

## Overview

Ahara Business runs Ahara Mail: a single-user authenticated mail application for
the Ahara business systems. The app receives mail for `ahara.io` through SES,
stores raw MIME in private S3, persists searchable text and metadata in shared
PostgreSQL, sends outbound mail through SES, and exposes mailbox operations in a
React SPA at `mail.ahara.io`.

Durable design choices are recorded in [ADRs](adr/README.md). The product
requirements live in [../mail-foundation-spec.md](../mail-foundation-spec.md).

## Runtime Shape

| Component | Runtime | Purpose |
| ---- | ---- | ---- |
| Frontend | Vite React + TypeScript on the platform website module | Authenticated mailbox, sent mail, contacts, routing, forwarding, and MFA flows |
| API | Rust Lambda behind the shared ALB | Authenticated JSON API for mailbox, contacts, domains, forwarding, outbound mail, and `/health` |
| Receipt gate | Rust Lambda invoked synchronously by SES | Stops unknown recipients and count floods before S3 storage |
| Ingest worker | Rust Lambda invoked asynchronously by SES | Fetches raw MIME, applies limits/security policy, persists mailbox rows, and enqueues forwarding work |
| Send worker | Rust Lambda invoked by EventBridge every minute | Claims outbound work, sends through SES, retries transient failures, and records final status |
| Feedback handler | Rust Lambda subscribed to SES SNS topics | Applies bounce/complaint feedback to outbound status and suppressions |
| Database | Shared PostgreSQL through the platform migration flow | Domains, addresses, contacts, messages, recipients, attachment refs, threads, forwarding rules, suppressions, and outbound work |
| Raw mail storage | Private project S3 bucket | Raw MIME retention under the `raw/` prefix with public access blocked and lifecycle controls |
| Auth | Shared Cognito app client | Public authenticated app access with TOTP setup and token validation |
| Terraform | Project-owned root | Frontend, API, Lambdas, SES identity/rules, raw-mail storage, SNS feedback, alarms, and DNS records |

## Mail Flow

Inbound mail enters through the active SES receipt rule set for `ahara.io`.
SES invokes the receipt gate before storage. Accepted recipients are written to
the raw-mail S3 bucket, then SES invokes ingest asynchronously with the S3
pointer. Ingest fetches the object, rejects oversize objects before parsing,
parses MIME, converts usable HTML fallbacks to plaintext, records recipients and
attachment metadata, applies spam/virus disposition, updates threads, and
creates normal mailbox rows only for accepted messages.

Outbound compose, reply, and forwarding requests create `messages` and
`outbound_work` rows. The send worker claims queued work in batches, checks
suppressions before send, constructs text/plain MIME with threading headers,
sends through SES, and records retry or final status. SES bounce and complaint
notifications flow through SNS to the feedback handler, which updates message
status and recipient suppressions.

## Security Boundaries

The frontend renders stored plaintext only. Sender-controlled HTML is stripped
at ingest, links are inert in the mailbox UI, display names and attachment
filenames are treated as untrusted display data, and quarantined or rejected
mail is excluded from normal mailbox reads.

The shared ALB validates Cognito tokens on authenticated API routes, and the API
also verifies Cognito access tokens before serving app data. `/health` is the
only unauthenticated API route. Mail logs and operational metrics omit message
bodies, full headers, and raw email addresses.

## Operational Controls

Inbound flood controls are split across the pre-S3 receipt gate and post-S3
ingest. The receipt gate limits accepted-recipient message counts, while ingest
enforces raw object size, MIME size, nesting depth, attachment count, and recent
raw-byte limits. Lambda reserved concurrency, CloudWatch alarms, SES reputation
alarms, S3 lifecycle controls, and scoped IAM policies bound cost and blast
radius.

## Code Boundaries

Backend business logic lives in the `backend/shared` crate so parsing, policy,
SQL, security disposition, forwarding, sending, and feedback behavior can be
tested outside Lambda glue. Lambda crates own handler setup and AWS integration.

Frontend code reads `window.__APP_CONFIG__` from the deployed website runtime
config, stores Cognito session state in the browser through the Cognito client,
and uses the typed API client in `frontend/src/api.ts` for all app data.

Terraform remains under `infrastructure/terraform/` and owns project mail
resources while reusing shared platform modules for website hosting, ALB API,
Cognito app-client creation, Lambda deployment, VPC discovery, and state.
