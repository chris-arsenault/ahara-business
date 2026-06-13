# ahara-business

Ahara Mail is the SES-backed mail foundation for Ahara business systems.

## Quickstart

```bash
cd frontend && pnpm install --frozen-lockfile
cd ..
make ci
```

The repository contains the Rust Lambda backend, React/TypeScript web client,
PostgreSQL mail model, project-owned mail infrastructure, deploy automation,
and documentation for operating the first Ahara mail domain.

## Documentation

| Topic | Link |
| ---- | ---- |
| Product spec | [mail-foundation-spec.md](mail-foundation-spec.md) |
| Architecture | [docs/architecture.md](docs/architecture.md) |
| Business Hub expansion | [docs/business-hub.md](docs/business-hub.md) |
| Operations | [docs/operations.md](docs/operations.md) |
| Deploy | [docs/deploy.md](docs/deploy.md) |
| Smoke check | [docs/smoke-check.md](docs/smoke-check.md) |
| Documentation index | [docs/README.md](docs/README.md) |
| Architecture decisions | [docs/adr/README.md](docs/adr/README.md) |
| Backlog | [docs/backlog.md](docs/backlog.md) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |
| Agent guide | [AGENTS.md](AGENTS.md) |
| Backend package | [backend/README.md](backend/README.md) |
| Frontend package | [frontend/README.md](frontend/README.md) |
| Terraform root | [infrastructure/terraform/README.md](infrastructure/terraform/README.md) |

## License

MIT. See [LICENSE](LICENSE).
