# Backlog

Planned-but-not-built work. Each item is a positive assertion of future-state behavior.

## Mail Capabilities

- Add IMAP or native mobile-client access.
- Add full-text search ranking and advanced search scopes.
- Add vectorized semantic mailbox search after the accepted-inbound read model
  is stable.

## Forwarding

- Extend forwarding audit views so they explain why a forwarding rule matched
  or did not match a specific inbound message.
- Add mailbox and thread-adjacent forwarding status showing queued, sent,
  failed, suppressed, and skipped forwarding outcomes.
- Add rule labels, notes, ownership, and contact/project context.
- Add forwarding attempt history tied to outbound work and feedback rows.

## Calendar And Booking

- Extend parsed ICS candidates with attendee metadata.
- Add contact-scoped calendar slices.
- Add availability windows and richer booking confirmation/cancellation flows.
- Add follow-up task generation from booking and calendar state changes.

## Basic Finance In/Out

- Add invoice records for expected money in, linked to contacts, bookings,
  messages, files, and notes.
- Add payment records for received money, with amount, method, date, reference,
  status, and optional invoice link.
- Add expense records for money out, with vendor/contact, category, receipt
  source, reimbursable state, and tax notes.
- Add a normalized ledger-event read model for simple in/out reporting.
- Add operator views for unpaid, overdue, paid, reimbursable, unreconciled, and
  period-total summaries.
- Add mailbox shortcuts for starting invoices, payments, or expenses from
  invoice emails, receipts, payment confirmations, and booking threads.
- Defer bank feeds, payment processing, tax filing, and double-entry accounting
  until the manual in/out workflow is stable.

## Business Hub Expansion

- Evolve this repo into Ahara Hub: the unified internal operator surface for
  mail, contacts, business workflows, and cross-app operator tools.
- Rename this repo/product from Ahara Business to Ahara Hub when mail is no
  longer the dominant surface.
- Build contact-centered activity timelines that join mail, files, bookings,
  payments, notes, and product resources.
- Add internal task and follow-up tracking linked to contacts, messages,
  bookings, files, and product resources.
- Add mentee-facing accounts backed by the shared access/grant model.
- Add external-recipient account binding and download surfaces for
  `ahara-access` grants.
- Add product integrations that create resources/assets/grants through
  `ahara-access`, starting with Tsonu Music pre-release sharing.
- Add access-event views in this repo so the operator can review allowed and
  denied asset download attempts.
- Add app-authorization audit/history and app catalog metadata beyond the
  current static operator role list.
