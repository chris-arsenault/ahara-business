# Agent Guide

Ahara Business is the mail foundation for the business/mentoring systems.

## Read first

| Topic | Link |
| ---- | ---- |
| Workspace overview | [README.md](README.md) |
| Documentation index | [docs/README.md](docs/README.md) |
| Product spec | [mail-foundation-spec.md](mail-foundation-spec.md) |
| Architecture | [docs/architecture.md](docs/architecture.md) |
| Architecture decisions | [docs/adr/README.md](docs/adr/README.md) |
| Backlog | [docs/backlog.md](docs/backlog.md) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |
| Platform integration | [../ahara/INTEGRATION.md](../ahara/INTEGRATION.md) |

## Critical rules

- Follow the platform integration contract in `../ahara/INTEGRATION.md`.
- Use the shared ALB, shared VPC, shared PostgreSQL/RDS, shared Cognito, shared state bucket, and tag/SSM discovery.
- Keep mail content rendering text-only. Ingest converts HTML to plaintext; UI surfaces stored plaintext and inert links.
- Keep SES, S3, SNS, and Lambda IAM least-privilege and scoped to project-owned resources.
- Add platform deployer primitives in `ahara-infra` before project Terraform depends on new AWS services.
- Register Cargo, npm, and Terraform members only after they build under CI.
- Never start a local dev server unless the user explicitly asks.
- Run `make ci` before handoff when files change.

## Code map

| Path | Purpose |
| ---- | ---- |
| `mail-foundation-spec.md` | Product and security requirements for the mail foundation |
| `backend/` | Rust Lambda workspace with API, receipt gate, ingest, send, feedback, and shared crates |
| `frontend/` | Vite React/TypeScript SPA for mailbox, contacts, routing, and auth |
| `db/migrations/` | Platform PostgreSQL schema, rollback, and seed files |
| `infrastructure/terraform/` | Project Terraform root for frontend, API, mail, storage, feedback, and alarms |
| `scripts/` | Local automation for deploy, integration tests, and CI guardrails |
| `docs/` | Architecture, operations, deploy, smoke checks, ADRs, and backlog |

## Commands

| Command | Purpose |
| ---- | ---- |
| `cd frontend && pnpm install --frozen-lockfile` | Install frontend dependencies from the lockfile |
| `make ci` | Run Rust lint/format/test, frontend lint/typecheck/test, and Terraform format checks |
| `make build` | Build Rust Lambda artifacts and the frontend bundle |
| `make deploy` | Run the local deploy script |
