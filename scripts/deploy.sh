#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TF_DIR="${ROOT_DIR}/infrastructure/terraform"

STATE_BUCKET="${STATE_BUCKET:-tfstate-559098897826}"
STATE_REGION="${STATE_REGION:-us-east-1}"

echo "Building Rust Lambdas..."
cd "${ROOT_DIR}/backend"
cargo lambda build --release

echo "Building frontend..."
cd "${ROOT_DIR}/frontend"
pnpm install --frozen-lockfile
pnpm run build

echo "Running database migrations..."
cd "${ROOT_DIR}"
db-migrate

echo "Deploying infrastructure..."
terraform -chdir="${TF_DIR}" init -reconfigure \
  -backend-config="bucket=${STATE_BUCKET}" \
  -backend-config="region=${STATE_REGION}" \
  -backend-config="use_lockfile=true"

terraform -chdir="${TF_DIR}" apply -auto-approve
