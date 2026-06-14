# Business Hub Expansion

This document describes the post-MVP direction for growing this repo from Ahara
Mail into Ahara Hub: the internal operator surface for mail, contacts, business
workflows, and cross-app operational tools.

## Direction

Ahara Hub lives in this repo. The current repo name is `ahara-business`; a
future rename to `ahara-hub` is appropriate once the app is broader than mail.
There is no planned separate `hub.ahara.io` application.

Each app keeps its service boundary, while this repo gives the operator one
coherent place to move between mail, contacts, calendars, forwarding, bookings,
money, and user access.

Initial facets:

- Mail, contacts, calendar, forwarding, booking, money, and mentee workflows in
  this repo.
- Operator-facing views over Tsonu Music catalog, releases, source masters,
  publishing, and analytics.
- Operator-facing access/user workflows, including platform app authorization
  administration that moved here from Ahara Portal.
- Shared files and grants from `ahara-access`.

## Operator Business Intent

The expansion is not "mail plus a file page". The intended product shape is a
small internal operating system for Ahara work where messages, contacts,
calendar events, forwarding rules, bookings, invoices, payments, expenses, and
notes are connected by stable records.

The core loop is:

- Mail receives the operational input: client messages, receipts, invoices,
  scheduling messages, booking changes, and forwarded notices.
- Forwarding and routing keep mail flowing to the right external destinations
  while preserving auditable internal state.
- Calendar and booking records turn scheduling mail into operator-visible work.
- Basic finance records track money in and money out against contacts,
  bookings, messages, and files.
- Contact activity ties the thread together so the operator can answer "what is
  happening with this person or project?" without hunting across surfaces.

Ahara Portal and Tsonu Music remain separate public products because their
primary users are recruiters and listeners. Operator-only workflows for those
products should appear in this repo over time so the internal view feels like
one platform with multiple facets.

## Shared Access Service

`ahara-access` is a reusable backend service, not an additional user-facing app
or login surface. It owns authorization records for people, groups, objects,
and files. Product apps reference shared grants instead of implementing their
own external-user permission systems.

Core records:

- `principal`: a Cognito-backed person, such as an operator, mentee, mastering
  engineer, collaborator, or reviewer.
- `audience`: a named group of principals.
- `resource`: a product-owned object reference, such as
  `tsonu-music:recording:recording_x` or
  `ahara-business:booking:booking_y`.
- `asset`: private file metadata and storage pointer.
- `grant`: permission binding for a principal or audience over a resource or
  asset, with optional expiry and revocation.
- `access_event`: audit event for grant creation, view, download, revoke, and
  denied access.

## Calendar And Booking

Calendar is a first-class business workflow, not just a display widget. It
should connect inbound scheduling mail, manual operator entries, contacts, and
future bookings.

Expected behavior:

- Parse calendar-like inbound mail and ICS attachments into candidate calendar
  events.
- Let the operator create, edit, cancel, and annotate internal events manually.
- Link events to contacts, messages, bookings, notes, and later finance rows.
- Track source information so an event created from mail can point back to the
  original message and attachment metadata.
- Distinguish tentative, confirmed, canceled, completed, and missed states.
- Add booking records on top of calendar events for sessions, calls, mentoring,
  or other work that needs confirmation and follow-up.
- Keep public booking pages out of scope until the internal calendar and
  booking workflow is correct.

Calendar records should be durable internal objects with stable IDs,
contact links, timestamps, notes, and visibility state. Later external
visibility can use grants, but the first operator surface should make the
calendar usable without public scheduling.

## Forwarding And Routing

Forwarding is part of the business operations layer because mail often needs to
flow to an outside system or person while Ahara keeps the auditable source of
truth.

Already-built forwarding supports address-scoped rules, sender filters, plus
tag filters, auth-pass requirements, asynchronous outbound delivery, bounce and
complaint feedback, UI management, and basic per-rule/per-message forwarding
status views.

Next behavior:

- Extend delivery audit views so they show why a message matched, did not
  match, forwarded, skipped, failed, or was suppressed.
- Show forwarding status beside mailbox messages and threads, not only in the
  forwarding operations view.
- Support rule notes, labels, and ownership so operational forwarding rules are
  understandable later.
- Add contact/project-aware forwarding views so forwarding can be understood in
  context, not only by domain and address.
- Track delivery attempts and terminal outcomes for forwarded messages without
  exposing full message bodies in logs or metrics.
- Keep forwarding constrained to accepted, pass-auth messages unless an
  explicit operator rule opts into a looser policy.

Forwarding should stay tied to the outbound queue and suppression model rather
than becoming an untracked mail relay.

## Basic Finance In/Out

Finance should start as lightweight money tracking, not a full accounting
system. The goal is to know what money is expected, what came in, what went
out, and which contact, booking, message, or file explains it.

Core records:

- `invoice`: money expected from a contact or project, with due date, status,
  line summary, notes, and source links.
- `payment`: money received, linked to an invoice when applicable, with amount,
  date, method, reference, and status.
- `expense`: money paid out, with vendor/contact, category, date, amount,
  receipt/source message links, and reimbursement/tax notes.
- `ledger_event`: normalized in/out event used for reporting and activity
  timelines.

Expected behavior:

- Create invoices, payments, and expenses manually from the operator UI.
- Start finance records from mailbox messages, such as invoice emails,
  receipts, payment confirmations, and booking discussions.
- Link finance rows to contacts, messages, bookings, files, and notes.
- Show simple in/out totals by period, status, contact, and category.
- Track unpaid, overdue, paid, void, reimbursable, and reconciled states.
- Attach or reference receipt/invoice files without making mail raw MIME the
  finance document store.
- Defer bank feeds, payment processing, tax filing, and double-entry accounting
  until the basic in/out workflow is proven.

## Build State

The shared backend and first operator surface are in place:

- `ahara-access` owns the durable access model, authenticated API, private asset
  bucket, browser-upload CORS, upload/download URL issuance, audience
  membership, revocation, and access-event recording.
- Ahara Business exposes the operator Files facet for managed uploads,
  principals, audiences, audience members, asset grants, and grant revocation.
- Ahara Business stores internal calendar events and booking records with
  contact and source-message links, exposes operator APIs for create/list/status
  updates, parses inbound ICS attachment candidates, and provides day, week, and
  agenda Calendar/Booking operator slices.
- Ahara Business exposes forwarding operations status for rules and recent
  matching inbound messages, derived from forwarding rules, accepted mail,
  outbound work, and feedback terminal states.
- Ahara Business owns the operator app-authorization surface, edits
  `ahara-business-app-authorizations`, reconciles shared Cognito users, and
  seeds `chris` with the same app roles formerly managed by Ahara Portal.

Next build slices:

- Extend calendar candidates with attendee metadata and contact-scoped calendar
  slices.
- Add forwarding match explanations, skipped/suppressed status, and thread or
  mailbox-adjacent forwarding indicators.
- Add lightweight finance tables and operator views for invoices, payments,
  expenses, and period totals.
- Link contacts and messages to calendar, forwarding, and finance records.
- Add authorization audit/history and app catalog metadata when the static role
  list becomes too small.
- Keep shared-file recipient/download work as a later access slice unless it is
  needed by one of the business workflows.

## Ahara Business Workflows

This repo should keep contacts as the internal CRM boundary and add business
objects around those contacts.

Planned workflow objects:

- Calendar events from ICS parsing and manual operator entry.
- Bookings, availability windows, confirmations, cancellations, and session
  notes.
- Invoices, payments, expenses, and lightweight ledger entries.
- Mentee account links from contacts to shared Cognito principals.
- Object grants delegated to the shared access service.
- Cross-record activity timeline for a contact, including mail, files,
  bookings, payments, and notes.
- Internal tasks and follow-ups linked to contacts, messages, bookings, or
  files.

The internal operator UI can ship first, but the data model should assume later
external visibility. Business records need stable ownership, contact links,
visibility state, and grant references from the beginning.

Mentee-facing work should use the same access model instead of a separate
permission system. A mentee can be a contact linked to a principal, with grants
to session notes, files, bookings, invoices, or later resources. The operator
view remains primary, but records should be ready for selective external
visibility.

## Operator Shell

The hub should feel like one product with multiple facets. This repo owns
shared login state, global navigation, app switching, and high-level activity
views for operator workflows. Product APIs continue to own their domain logic.

The shell should avoid becoming a direct database client. It should compose
typed APIs from the product services and shared access service.

Ahara Portal and Tsonu Music remain separate products because their primary
audiences are external: recruiters and listeners. Operator-only workflows should
move toward this repo over time.

## Non-Goals

- Do not expose `tsonu-music` private master bucket objects directly to external
  recipients.
- Do not use unlisted public music pages as the file-sharing security model.
- Do not store general-purpose shared files in Ahara Mail raw MIME storage.
- Do not duplicate grant logic independently in each app.
- Do not create a separate hub application or domain unless this repo becomes
  too broad to operate cleanly.
- Do not require microfrontends for the first hub version; shared UI patterns
  and API-backed modules are enough.
