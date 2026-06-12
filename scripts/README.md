# Scripts

Local project automation.

`scripts/deploy.sh` is a parameterless local deploy path. It builds Rust Lambda
artifacts, builds the frontend bundle, runs the platform `db-migrate` flow,
initializes Terraform against the shared state bucket, and applies Terraform.

`scripts/run-backend-integration-tests.sh` is the parameterless PostgreSQL
integration suite used by local `make ci` and the shared CI workflow's
`rust_extra_ci_commands` hook.

The script assumes local access to the platform tooling and does not start a
development server.

See [docs/deploy.md](../docs/deploy.md) for the full local and CI deploy flow.
