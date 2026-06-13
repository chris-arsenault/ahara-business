# Business Hub Expansion

This document describes the post-MVP direction for growing this repo from Ahara
Mail into Ahara Hub: the internal operator surface for mail, contacts, business
workflows, and cross-app operational tools.

## Direction

Ahara Hub lives in this repo. The current repo name is `ahara-business`; a
future rename to `ahara-hub` is appropriate once the app is broader than mail.
There is no planned separate `hub.ahara.io` application.

Each app keeps its service boundary, while this repo gives the operator one
coherent place to move between mail, contacts, music, files, bookings, money,
and user access.

Initial facets:

- Mail, contacts, calendar, booking, money, and mentee workflows in this repo.
- Operator-facing views over Tsonu Music catalog, releases, source masters,
  publishing, and analytics.
- Operator-facing access/user workflows that may start from Ahara Portal's
  existing admin surface and move here as the hub matures.
- Shared files and grants from `ahara-access`.

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

## Secure File Sharing

Limited-audience file sharing is a platform capability, not a mail attachment
feature and not a Tsonu-only feature. The primary initial use case is sharing
Tsonu pre-release source or review files with a mastering engineer without
publishing them.

Expected behavior:

- Files live in private service-owned storage or are copied into a private
  sharing package from a product-owned private bucket.
- External recipients authenticate through shared Cognito.
- Downloads use short-lived CloudFront signed URLs or equivalent short-lived
  delivery credentials.
- Revocation prevents future URL issuance.
- Every list, view, and download attempt writes an audit event.
- Public product APIs never expose private bucket names, keys, version IDs, or
  upload ETags.

## Build State

The shared backend and first operator surface are in place:

- `ahara-access` owns the durable access model, authenticated API, private asset
  bucket, browser-upload CORS, upload/download URL issuance, audience
  membership, revocation, and access-event recording.
- Ahara Business exposes the operator Files facet for managed uploads,
  principals, audiences, audience members, asset grants, and grant revocation.

Next build slices:

- Add recipient account binding and download surfaces so external collaborators
  can authenticate and retrieve granted files without operator-only tooling.
- Add product integrations, starting with Tsonu Music pre-release sharing.
- Link contacts and future business records to access principals and grants.

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

The internal operator UI can ship first, but the data model should assume later
external visibility. Business records need stable ownership, contact links,
visibility state, and grant references from the beginning.

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
