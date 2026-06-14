# Backend

Rust Lambda workspace for Ahara Mail.

## Workspace Members

| Member | Purpose |
| ---- | ---- |
| `shared` | Shared mail, auth, app authorization, database, routing, inbound, outbound, feedback, and observability logic |
| `api` | Axum/lambda_http API Lambda for health, user context, domains, contacts, mailbox, outbound, forwarding, and app-authorization routes |
| `receipt-gate` | SES synchronous receipt Lambda for accepted-recipient checks and count-based flood control |
| `ingest` | SES async ingest Lambda for raw MIME fetch, parsing, security disposition, persistence, and forwarding enqueue |
| `send-worker` | Scheduled outbound worker for SES send, retry, suppression checks, and status updates |
| `feedback-handler` | SNS feedback handler for SES bounce and complaint suppression/status updates |

The Lambda crates keep AWS handler glue thin. Reusable behavior belongs in
`shared` and is covered by unit and PostgreSQL integration tests.

## Verification

```bash
cargo fmt -- --check
CARGO_TARGET_DIR=target-clippy cargo clippy --workspace --all-targets --release -- -D warnings -W clippy::cognitive_complexity
CARGO_TARGET_DIR=target-cov cargo test --release --lib
../scripts/run-backend-integration-tests.sh
```
