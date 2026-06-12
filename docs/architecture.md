# Architecture

## Overview

Ahara Business is the mail foundation project for the Ahara business systems.
The current M0 architecture is a runnable platform scaffold: Rust Lambda
binaries, a React/TypeScript SPA, a project Terraform root, shared CI, and
platform registration. The product target remains the SES-backed text-only mail
system described in `mail-foundation-spec.md` and `MAIL-FOUNDATION-PLAN.md`.

## Current Runtime Shape

| Component | Runtime | Purpose |
| ---- | ---- | ---- |
| Frontend | Vite React + TypeScript SPA | Authenticated-app shell that reads platform runtime config |
| API | Rust Lambda behind shared ALB | Minimal Axum/lambda_http router with unauthenticated `/health` |
| Ingest worker | Rust Lambda | No-op event handler scaffold |
| Send worker | Rust Lambda | No-op event handler scaffold |
| Feedback handler | Rust Lambda | No-op event handler scaffold |
| Database | Shared PostgreSQL registration | Platform migration target for future schema work |
| Auth | Shared Cognito app client | Public authenticated web path through the platform |
| Terraform | Project root | Website, Cognito app, ALB API, standalone Lambdas, outputs |

## Platform Integration

The project follows the Ahara platform contracts from `../ahara/INTEGRATION.md`:
shared state, shared ALB, shared Cognito, shared RDS migration registration, and
Terraform module reuse from `ahara-tf-patterns`. M0 also registers the
`ahara-business` deployer role and database in `ahara-infra`, including the SES
and private-storage deployer primitives required by later mail infrastructure.

The project Terraform currently owns the website, Cognito app client, API
Lambda behind the shared ALB, and three standalone worker Lambdas. SES
identities, receipt rules, raw-mail S3 storage, SNS topics, and mail event
sources are planned for M3.

## Text-Only Rendering

ADR-0002 sets the UI boundary: the public app surface is authenticated through
Cognito, and mail content rendering stays text-only. The frontend scaffold does
not render mail content yet. When ingest is implemented, HTML email is converted
to readable plaintext, stripping markup, scripts, styles, and remote references,
and the UI renders stored plaintext only.

## Code Boundaries

Backend code lives in a Cargo workspace so API and worker glue can share tested
library behavior. Frontend code lives in a Vite package and reads
`window.__APP_CONFIG__` from the website runtime config. Database migrations live
under `db/migrations/` and remain intentionally empty until M2 creates schema
SQL. Terraform stays in `infrastructure/terraform/` and uses platform modules
rather than hand-owned shared resources.

## Target Mail Flow

The later mail system receives through SES, stores raw MIME in private S3,
persists searchable text records in shared PostgreSQL, sends outbound mail
through SES, and processes bounce/complaint SNS feedback. Those flows are not
implemented in M0; they are phased in by the implementation plan.

Raw MIME remains private S3 data in the target architecture. Logs omit message
bodies and full headers.
