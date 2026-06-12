# Agent Guide

Ahara Business is the mail foundation for the business/mentoring systems.

## Read first

| Topic | Link |
| ---- | ---- |
| Workspace overview | [README.md](README.md) |
| Product spec | [mail-foundation-spec.md](mail-foundation-spec.md) |
| Architecture | [docs/architecture.md](docs/architecture.md) |
| Implementation plan | [MAIL-FOUNDATION-PLAN.md](MAIL-FOUNDATION-PLAN.md) |
| Architecture decisions | [docs/adr/README.md](docs/adr/README.md) |
| Backlog | [docs/backlog.md](docs/backlog.md) |
| Platform integration | [../ahara/INTEGRATION.md](../ahara/INTEGRATION.md) |

## Critical rules

- Follow the platform integration contract in `../ahara/INTEGRATION.md`.
- Use the shared ALB, shared VPC, shared PostgreSQL/RDS, shared Cognito, shared state bucket, and tag/SSM discovery.
- Keep mail content rendering text-only. Ingest converts HTML to plaintext; UI surfaces stored plaintext and inert links.
- Keep SES, S3, SNS, and Lambda IAM least-privilege and scoped to project-owned resources.
- Add platform deployer primitives in `ahara-infra` before project Terraform depends on new AWS services.
- Register only buildable Cargo/npm/Terraform members. Keep future homes README-only until code exists.
- Never start a local dev server unless the user explicitly asks.
- Run `make ci` before handoff when files change.

## Code map

| Path | Purpose |
| ---- | ---- |
| `mail-foundation-spec.md` | Product and security requirements for the mail foundation |
| `MAIL-FOUNDATION-PLAN.md` | Milestone-level implementation plan |
| `backend/` | Rust Lambda workspace with API, worker, and shared crates |
| `frontend/` | Vite React/TypeScript SPA scaffold |
| `db/migrations/` | Platform PostgreSQL migration directory; intentionally empty until M2 |
| `infrastructure/terraform/` | Project Terraform root for the M0 app scaffold |
| `scripts/` | Local automation, including parameterless deploy |
| `docs/` | Architecture, ADRs, and backlog |

## Commands

| Command | Purpose |
| ---- | ---- |
| `cd frontend && pnpm install --frozen-lockfile` | Install frontend dependencies from the lockfile |
| `make ci` | Run Rust lint/format/test, frontend lint/typecheck/test, and Terraform format checks |
| `make build` | Build Rust Lambda artifacts and the frontend bundle |
| `make deploy` | Run the local deploy script |
