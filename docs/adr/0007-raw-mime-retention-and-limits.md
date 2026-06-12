# 0007 - Raw MIME Retention And Limits

- Status: Accepted
- Date: 2026-06-12

## Context

Raw MIME is the audit and recovery source for messages, including attachment
bytes that the app records only as metadata. The storage policy needs enough
retention for operational recovery while bounding S3 growth and inbound flood
cost.

## Decision

Keep current raw MIME objects for 365 days, keep noncurrent versions for 30
days, abort incomplete multipart uploads after one day, reject raw objects over
10 MiB before download, and reject accepted inbound traffic above 50 MiB of
recent raw bytes per hour.

## Alternatives considered

- **Short raw retention window** - Lowers S3 cost, but weakens audit and
  recovery for business correspondence and attachment metadata.
- **Unbounded raw retention and size** - Maximizes recovery, but gives inbound
  abuse and attachment floods too much cost leverage.
- **Parse first, then size-check** - Preserves more diagnostic context for
  oversized messages, but spends Lambda memory and CPU on data that policy will
  reject.

## Consequences

The raw MIME bucket remains useful for recovery while lifecycle controls bound
long-term storage. Ingest performs cheap object metadata checks before reading
large bodies, and hourly raw-byte accounting gives the application a second
cost-control layer after the receipt gate.
