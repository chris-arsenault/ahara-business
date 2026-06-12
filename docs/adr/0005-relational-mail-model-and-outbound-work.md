# 0005 - Relational Mail Model And Outbound Work

- Status: Accepted
- Date: 2026-06-12

## Context

Ahara Mail needs queryable mailbox reads, thread views, routing records,
recipient-level display, forwarding, suppressions, and asynchronous outbound
delivery. The schema also needs to support PostgreSQL integration tests and
remain understandable to agents working from a checkout.

## Decision

Store recipients as normalized `recipients` rows and store outbound queue state
in a dedicated `outbound_work` table linked to `messages`.

## Alternatives considered

- **Recipient arrays on `messages`** - Lower join count for simple message reads,
  but weaker validation, harder per-recipient querying, and less room for
  display metadata.
- **Fold outbound queue fields entirely into `messages`** - Fewer tables, but it
  mixes mailbox state with worker locking, retry, idempotency, and source
  message references.

## Consequences

Mailbox and outbound queries join more tables, but data constraints stay local
to the concepts they validate. The send worker can claim and retry work without
overloading the user-facing message row, and recipient metadata remains
available for both inbound and outbound views.
