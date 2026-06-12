# M0 - Project Scaffold And Platform Registration

This expands M0 from `MAIL-FOUNDATION-PLAN.md` into execution-ready steps. Run only these steps for M0, in order. No M0 step carries a `[DECISION]` tag because the required architectural choices are already settled by ADRs 0001-0004.

Exit gate: `make ci` green in `ahara-business`; affected `ahara-infra` changes are present and `make ci` is green there; no README-only placeholder homes remain for buildable workspace members.

1. Add Ahara deployer policy primitives for SES and private S3 storage.
   - File(s): `../ahara-infra/infrastructure/terraform/control/modules/policy-library/ses/iam.tf`, `../ahara-infra/infrastructure/terraform/control/modules/policy-library/ses/variables.tf`, `../ahara-infra/infrastructure/terraform/control/modules/policy-library/ses/outputs.tf`, `../ahara-infra/infrastructure/terraform/control/modules/policy-library/s3-private-storage/iam.tf`, `../ahara-infra/infrastructure/terraform/control/modules/policy-library/s3-private-storage/variables.tf`, `../ahara-infra/infrastructure/terraform/control/modules/policy-library/s3-private-storage/outputs.tf`, `../ahara-infra/infrastructure/terraform/control/modules/managed-project/policy-map.tf`, `../ahara-infra/infrastructure/terraform/control/modules/managed-project/variables.tf`.
   - Reference behavior: ADR-0004 keeps mail resources project-owned while deployer IAM remains least-privilege. Reuse the existing `sns`, `s3-website`, and `db-migrate` policy-library module shape: each primitive has `variables.tf`, `iam.tf`, and `outputs.tf`, and exposes `policy_json` for `managed-project/policy-map.tf`.
   - Change: add a `ses` primitive scoped to the SES identity, receipt, configuration-set, and feedback resources needed by project Terraform, using account/region/prefix constraints wherever SES supports resource scoping. Add an `s3-private-storage` primitive scoped to `arn:aws:s3:::${var.prefix}-*` buckets and objects with bucket creation, versioning, encryption, public-access-block, lifecycle, bucket policy, notification, and object CRUD actions, but no website-hosting/public ACL defaults. Register both primitives in `policy-map.tf` and update the valid primitive description in `variables.tf`.
   - Verify: from `../ahara-infra`, run `terraform fmt -check -recursive infrastructure/terraform/control` plus `rg '"ses"' infrastructure/terraform/control/modules/managed-project/policy-map.tf` and `rg '"s3-private-storage"' infrastructure/terraform/control/modules/managed-project/policy-map.tf`. Red before: the primitive module directories and policy-map keys do not exist. Green after.

2. Register the `ahara-business` deployer role.  [depends on #1]
   - File(s): `../ahara-infra/infrastructure/terraform/control/project-ahara-business.tf`.
   - Reference behavior: `../ahara/INTEGRATION.md` Step 1 and existing files such as `../ahara-infra/infrastructure/terraform/control/project-tastebase.tf`. Prefix and state key must match platform convention: `prefix = "ahara-business"`, `state_key_prefix = "projects/ahara-business"`, and `allowed_repos = ["ahara-business"]`.
   - Change: add a `module "project_ahara_business"` managed-project block with `module_bundles = ["website", "alb-api", "cognito-app", "lambda"]` and `policy_modules = ["terraform-state", "db-migrate", "sns", "ses", "s3-private-storage"]`.
   - Verify: from `../ahara-infra`, run `terraform fmt -check -recursive infrastructure/terraform/control` plus `rg 'module "project_ahara_business"' infrastructure/terraform/control/project-ahara-business.tf` and `rg '"ses"|\"s3-private-storage\"' infrastructure/terraform/control/project-ahara-business.tf`. Red before: project registration file does not exist. Green after.

3. Register the project database.
   - File(s): `../ahara-infra/infrastructure/terraform/services/db-migrate.tf`.
   - Reference behavior: `../ahara/INTEGRATION.md` Step 5 and the current `migration_projects` map. Project keys may contain hyphens, but PostgreSQL database names should use underscores.
   - Change: add `"ahara-business" = { db_name = "ahara_business" }` to `var.migration_projects`. Do not add SQL grants, users, roles, or databases in this repo.
   - Verify: from `../ahara-infra`, run `terraform fmt -check -recursive infrastructure/terraform/services` plus `rg '"ahara-business"\s*=\s*\{ db_name = "ahara_business" \}' infrastructure/terraform/services/db-migrate.tf`. Red before: the registration line is absent. Green after.

4. Declare platform stack metadata and shared CI.
   - File(s): `platform.yml`, `.github/workflows/ci.yml`.
   - Reference behavior: `../ahara/CI-WORKFLOW.md` requires `platform.yml` with `stack` entries and `rust_artifacts` when Rust is present; the caller workflow should use `chris-arsenault/ahara/.github/workflows/ci.yml@main`.
   - Change: update `platform.yml` to declare `stack: [rust, typescript, terraform]`, `migrations: db/migrations`, and `rust_artifacts.lambdas` for `api`, `ingest`, `send-worker`, and `feedback-handler`. Add the minimal shared workflow caller with `id-token: write`, `contents: read`, and `secrets: inherit`.
   - Verify: run `python3 - <<'PY'\nimport yaml\ncfg=yaml.safe_load(open('platform.yml'))\nassert cfg['project']=='ahara-business'\nassert cfg['prefix']=='ahara-business'\nassert cfg['migrations']=='db/migrations'\nassert cfg['stack']==['rust','typescript','terraform']\nassert cfg['rust_artifacts']['lambdas']==['api','ingest','send-worker','feedback-handler']\nPY` and `rg 'chris-arsenault/ahara/.github/workflows/ci.yml@main' .github/workflows/ci.yml`. Red before: `stack`, `rust_artifacts`, and the workflow file are absent. Green after.

5. Create the buildable Rust Lambda workspace.
   - File(s): `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/shared/Cargo.toml`, `backend/shared/src/lib.rs`, `backend/api/Cargo.toml`, `backend/api/src/main.rs`, `backend/ingest/Cargo.toml`, `backend/ingest/src/main.rs`, `backend/send-worker/Cargo.toml`, `backend/send-worker/src/main.rs`, `backend/feedback-handler/Cargo.toml`, `backend/feedback-handler/src/main.rs`.
   - Reference behavior: ADR-0001 chooses Rust Lambdas and shared library crates. `../ahara-tf-patterns/modules/alb-api` and `../ahara-tf-patterns/modules/lambda` consume `backend/target/lambda/<bin>/bootstrap` artifacts built by the shared workflow. Adjacent projects use a `backend/` workspace with `resolver = "2"` and edition 2024.
   - Change: replace the README-only backend placeholder with a real workspace. Add a `shared` library with one smoke-tested exported function or type. Add an `api` Lambda binary with a minimal authenticated-ready Axum/lambda_http router and `/health`. Add no-op `ingest`, `send-worker`, and `feedback-handler` lambda_runtime binaries that parse `serde_json::Value`, log a non-PII message, and return success. Do not implement mail parsing, sending, database schema, or SES/S3/SNS behavior in M0.
   - Verify: run `cd backend && cargo fmt -- --check && CARGO_TARGET_DIR=target-clippy cargo clippy --release -- -D warnings -W clippy::cognitive_complexity && CARGO_TARGET_DIR=target-cov cargo test --release --lib`. Red before: `backend/Cargo.toml` and the crate symbols do not exist. Green after.

6. Create the buildable React/TypeScript frontend.
   - File(s): `frontend/package.json`, `frontend/pnpm-lock.yaml`, `frontend/index.html`, `frontend/vite.config.ts`, `frontend/tsconfig.json`, `frontend/tsconfig.app.json`, `frontend/tsconfig.node.json`, `frontend/eslint.config.js`, `frontend/src/main.tsx`, `frontend/src/App.tsx`, `frontend/src/App.test.tsx`, `frontend/src/config.ts`, `frontend/src/vite-env.d.ts`, `frontend/src/index.css`.
   - Reference behavior: ADR-0001 chooses a React/TypeScript SPA; ADR-0002 requires the UI to stay text-only for mail content. `../ahara/CI-WORKFLOW.md` installs pnpm 10.29.3 and runs `pnpm install --frozen-lockfile`, ESLint, TypeScript, optional Vitest, and `pnpm run build`.
   - Change: replace the README-only frontend placeholder with a minimal Vite/React app that reads `window.__APP_CONFIG__`, renders a neutral authenticated-app shell, and contains no mail rendering or direct API behavior. Use the existing Ahara ESLint standards package pattern. Include Vitest so the shared workflow's optional test path is deterministic, and commit the generated `pnpm-lock.yaml`.
   - Verify: run `cd frontend && pnpm install --frozen-lockfile && pnpm exec eslint . && pnpm exec tsc --noEmit && pnpm exec vitest run --coverage && pnpm run build`. Red before: `frontend/package.json` and source files do not exist. Green after.

7. Create the project Terraform root.  [depends on #4, #5, #6]
   - File(s): `infrastructure/terraform/main.tf`, `infrastructure/terraform/locals.tf`, `infrastructure/terraform/ssm.tf`, `infrastructure/terraform/cognito.tf`, `infrastructure/terraform/frontend.tf`, `infrastructure/terraform/lambdas.tf`, `infrastructure/terraform/outputs.tf`.
   - Reference behavior: `../ahara/INTEGRATION.md` Steps 2, 4, 5, 6, and 7; `../ahara-tf-patterns/modules/platform-context`, `website`, `alb-api`, `cognito-app`, and `lambda`; adjacent Tastebase Terraform. Use shared state key `projects/ahara-business.tfstate`, hostnames `ahara-business.ahara.io` and `api.ahara-business.ahara.io`, and an unused ALB listener priority in the 300+ consumer range.
   - Change: add Terraform provider/backend/default tags, platform context, per-project DB SSM parameter lookups, shared Cognito app client, website module, API module with an authenticated catch-all API route plus unauthenticated `/health` route, and standalone no-op Lambdas for `ingest`, `send-worker`, and `feedback-handler` reusing the API role. Do not add SES, raw-mail S3 bucket, SNS topics, receipt rules, or mail event sources in M0; those belong to M3.
   - Verify: run `terraform fmt -check -recursive infrastructure/terraform/` plus `rg 'projects/ahara-business.tfstate' infrastructure/terraform/main.tf`, `rg 'api\\.ahara-business\\.ahara\\.io' infrastructure/terraform/locals.tf`, and `rg 'priority\\s*=\\s*32[0-9]' infrastructure/terraform/lambdas.tf`. Red before: Terraform root files and priority allocation are absent. Green after.

8. Add the local deploy script.  [depends on #5, #6, #7]
   - File(s): `scripts/deploy.sh`.
   - Reference behavior: `../ahara/INTEGRATION.md` Step 3 and adjacent `scripts/deploy.sh` files. CI replicates these steps explicitly and does not call this script.
   - Change: replace the README-only scripts placeholder with an executable, parameterless deploy script that builds Rust Lambdas, builds the frontend, runs `db-migrate`, initializes Terraform against `tfstate-559098897826`, and applies Terraform. Keep it local-only and do not start any dev server.
   - Verify: run `bash -n scripts/deploy.sh && test -x scripts/deploy.sh && rg 'db-migrate' scripts/deploy.sh && rg 'terraform .*apply -auto-approve' scripts/deploy.sh`. Red before: `scripts/deploy.sh` does not exist. Green after.

9. Expand `make ci` to mirror the shared workflow.  [depends on #5, #6, #7]
   - File(s): `Makefile`.
   - Reference behavior: `../ahara/CI-WORKFLOW.md` Makefile section and adjacent Tastebase Makefile. Local CI should cover Rust format/clippy/test, TypeScript lint/typecheck/test, and Terraform format.
   - Change: replace the docs-only `ci` target with `lint`, `fmt`, `typecheck`, `test`, `terraform-fmt-check`, `build`, and `deploy` targets. Keep `build` separate from `ci`. Use `CARGO_TARGET_DIR=target-clippy` for clippy and `CARGO_TARGET_DIR=target-cov cargo test --release --lib` for Rust tests.
   - Verify: run `rg 'CARGO_TARGET_DIR=target-clippy cargo clippy --release -- -D warnings -W clippy::cognitive_complexity' Makefile`, `rg 'pnpm exec eslint \.' Makefile`, `rg 'terraform fmt -check -recursive infrastructure/terraform/' Makefile`, then `make ci`. Red before: the grep checks fail against the docs-only Makefile. Green after.

10. Update repo docs from reserved scaffold to runnable scaffold.  [depends on #4, #5, #6, #7, #8, #9]
   - File(s): `README.md`, `AGENTS.md`, `docs/architecture.md`, `backend/README.md`, `frontend/README.md`, `db/migrations/README.md`, `infrastructure/terraform/README.md`, `scripts/README.md`.
   - Reference behavior: `repo-docs` conventions keep root docs as indexes and current-state docs in `docs/`. M0 exit requires no README-only placeholder homes for buildable workspace members.
   - Change: update the code map and architecture to describe the actual buildable scaffold. Remove or rewrite placeholder language such as "Reserved home" where a real package/root now exists. Keep database migrations documented as an intentionally empty migration home until M2 creates schema SQL.
   - Verify: run `! rg 'Reserved home|docs-only validation|placeholder workspace' README.md AGENTS.md docs backend frontend infrastructure scripts`. Red before: current scaffold docs contain reserved-home/docs-only wording. Green after.

11. Run the M0 exit gate.  [depends on #1, #2, #3, #4, #5, #6, #7, #8, #9, #10]
   - File(s): none; this is the phase gate.
   - Reference behavior: M0 exit in `MAIL-FOUNDATION-PLAN.md` and `../ahara-infra/AGENTS.md`.
   - Change: no code change. This step exists to prove the phase is complete after the preceding edits.
   - Verify: run `make ci` from `/home/dev/repos/ahara-business`, then run `make ci` from `/home/dev/repos/ahara-infra`, then capture `git status --short` in both repos. Red before: M0 has not yet produced buildable project files or platform registrations. Green after.
