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

## Tax/Audit Finance

- Add CSV export for expense allocation and client receivable status by tax
  year.
- Add receipt attachment shortcuts from mailbox messages and shared files into
  expense records.
- Add recurring-expense review reminders so cloud, AI, internet, software, and
  other ongoing costs are revisited before tax prep.
- Add contact/project filters for client receivables and expense records.
- Add finance rows to the future contact activity timeline.
- Keep payment processing, payment credentials, checkout links, bank feeds, tax
  filing, and double-entry accounting out of this app.

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
