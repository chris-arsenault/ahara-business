#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${ROOT_DIR}"

# Lambda's SDK adds a recursion-detection header while presigning; exercise the
# browser header contract with that runtime context and dummy test credentials.
AWS_LAMBDA_FUNCTION_NAME=attachment-upload-test \
  _X_AMZN_TRACE_ID='Root=1-00000000-000000000000000000000000;Parent=0000000000000000;Sampled=0' \
  cargo test --manifest-path backend/Cargo.toml -p shared --lib \
  attachment_storage::tests::signed_upload_constrains_length_checksum_and_expires

cargo test --manifest-path backend/Cargo.toml -p shared --test mail_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test inbound_ingest_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test mailbox_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test outbound_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test s3_attachments -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test forwarding_model -- --nocapture
cargo test --manifest-path backend/Cargo.toml -p shared --test feedback_model -- --nocapture
