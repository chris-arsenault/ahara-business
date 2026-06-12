.PHONY: ci lint fmt typecheck test file-length-check terraform-fmt-check build deploy

ci: lint fmt typecheck test file-length-check terraform-fmt-check

lint:
	cd backend && CARGO_TARGET_DIR=target-clippy cargo clippy --workspace --all-targets --release -- -D warnings -W clippy::cognitive_complexity
	cd frontend && pnpm exec eslint .

fmt:
	cd backend && cargo fmt -- --check
	cd frontend && pnpm exec prettier --check .

typecheck:
	cd frontend && pnpm exec tsc --noEmit

test:
	cd backend && CARGO_TARGET_DIR=target-cov cargo test --release --lib
	scripts/run-backend-integration-tests.sh
	cd frontend && pnpm exec vitest run --coverage

file-length-check:
	scripts/check-file-lengths.sh

terraform-fmt-check:
	terraform fmt -check -recursive infrastructure/terraform/

build:
	cd backend && cargo lambda build --release
	cd frontend && pnpm run build

deploy:
	scripts/deploy.sh
