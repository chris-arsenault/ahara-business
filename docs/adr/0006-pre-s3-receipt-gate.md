# 0006 - Pre-S3 Receipt Gate

- Status: Accepted
- Date: 2026-06-12

## Context

SES can write accepted mail directly to S3 and invoke an asynchronous ingest
Lambda, but that shape stores attacker-controlled messages before application
routing or count limits run. Ahara Mail routes only selected addresses and keeps
cost controls load-bearing for public MX activation.

## Decision

Invoke a synchronous receipt-gate Lambda before the SES S3 action. The gate
accepts configured recipients, supports plus-address variants for those
recipients, applies rolling count limits, and returns `STOP_RULE_SET` for
unknown recipients or count-limit blocks.

## Alternatives considered

- **Post-S3 rejection in ingest only** - Simpler receipt rules, but raw S3
  writes and async Lambda invokes happen for unknown recipients and dictionary
  attacks.
- **SES receipt rule recipients only** - Useful static filtering, but it does
  not enforce rolling limits or share logic with application-side accepted
  address configuration.

## Consequences

Unknown-recipient and high-count floods stop before raw storage. The receipt
gate stays intentionally small: it does not fetch S3, parse MIME, persist
mailbox rows, send mail, or log sender-controlled content.
