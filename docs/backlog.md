# Backlog

Planned-but-not-built work. Each item is a positive assertion of future-state behavior.

## Mail Capabilities

- Add IMAP or native mobile-client access.
- Add full-text search ranking and advanced search scopes.
- Add vectorized semantic mailbox search after the accepted-inbound read model
  is stable.

## Forwarding

- Add per-rule delivery audit views that show why a forwarding rule matched or
  did not match a specific inbound message.

## Business Hub Expansion

- Evolve this repo into Ahara Hub: the unified internal operator surface for
  mail, contacts, business workflows, and cross-app operator tools.
- Rename this repo/product from Ahara Business to Ahara Hub when mail is no
  longer the dominant surface.
- Add external-recipient account binding and download surfaces for
  `ahara-access` grants.
- Add product integrations that create resources/assets/grants through
  `ahara-access`, starting with Tsonu Music pre-release sharing.
- Add access-event views in this repo so the operator can review allowed and
  denied asset download attempts.
- Add contact-linked calendar and ICS handling.
- Add booking workflows with internal operator controls before public booking
  pages.
- Add money tracking for invoices, payments, expenses, and lightweight ledger
  events.
- Add mentee-facing accounts backed by the shared access/grant model.
