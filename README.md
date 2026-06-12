# ahara-business

Mail foundation scaffold for the Ahara business systems.

## Quickstart

```bash
cd frontend && pnpm install --frozen-lockfile
cd ..
make ci
```

The M0 scaffold includes a Rust Lambda workspace, React/TypeScript SPA,
project Terraform root, platform migration directory, local deploy script, and
shared CI workflow. The mail transport implementation remains tracked in
[MAIL-FOUNDATION-PLAN.md](MAIL-FOUNDATION-PLAN.md).

## Documentation

| Topic | Link |
| ---- | ---- |
| Product spec | [mail-foundation-spec.md](mail-foundation-spec.md) |
| Architecture | [docs/architecture.md](docs/architecture.md) |
| Operations | [docs/operations.md](docs/operations.md) |
| Deploy | [docs/deploy.md](docs/deploy.md) |
| Smoke check | [docs/smoke-check.md](docs/smoke-check.md) |
| Implementation plan | [MAIL-FOUNDATION-PLAN.md](MAIL-FOUNDATION-PLAN.md) |
| Documentation index | [docs/README.md](docs/README.md) |
| Architecture decisions | [docs/adr/README.md](docs/adr/README.md) |
| Backlog | [docs/backlog.md](docs/backlog.md) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |
| Agent guide | [AGENTS.md](AGENTS.md) |

## License

MIT. See [LICENSE](LICENSE).
