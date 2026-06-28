#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${ROOT_DIR}"

cargo test --manifest-path backend/Cargo.toml -p shared --test mail_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test inbound_ingest_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test mailbox_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test outbound_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test forwarding_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test feedback_model -- --nocapture
