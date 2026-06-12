# Backend

Rust Lambda workspace for the Ahara Business app scaffold.

## Workspace Members

| Member | Purpose |
| ---- | ---- |
| `shared` | Shared library crate with smoke-tested exported helpers |
| `api` | Axum/lambda_http API Lambda with `/health` |
| `ingest` | No-op Lambda event handler scaffold |
| `send-worker` | No-op Lambda event handler scaffold |
| `feedback-handler` | No-op Lambda event handler scaffold |

The worker binaries parse generic JSON Lambda events, emit non-PII logs, and
return success. Mail parsing, sending, persistence, and SES/S3/SNS event wiring
belong to later milestones.

## Verification

```bash
cargo fmt -- --check
CARGO_TARGET_DIR=target-clippy cargo clippy --release -- -D warnings -W clippy::cognitive_complexity
CARGO_TARGET_DIR=target-cov cargo test --release --lib
```
