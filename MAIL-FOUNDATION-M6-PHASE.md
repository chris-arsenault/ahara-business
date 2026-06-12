# M6 - Text-Only Mail UI And Read API

This phase expands only M6 from `MAIL-FOUNDATION-PLAN.md`. The goal is to
deliver the authenticated mailbox reading workflow: backend read APIs over
accepted inbound mail, React auth/bootstrap/runtime config, mailbox list,
thread/message detail, read/unread, contact association, routing-policy admin,
and search. The UI renders stored plaintext only.

M6 scope guard:
- Do not add compose, reply, outbound send, forwarding, suppression, bounce, or
  complaint workflows. Those belong to M7.
- Do not add MX records, activate SES receipt rules, or change inbound mail
  routability.
- Do not render sender-controlled HTML, use `dangerouslySetInnerHTML`, or turn
  message-body URLs into active links by default.
- Do not expose quarantined or rejected messages through normal mailbox list,
  search, thread-detail, or message-detail routes.
- Do not add raw MIME/original download routes in M6. Show attachment metadata
  only.
- Do not create a marketing/landing page. The first authenticated surface is
  the usable mailbox.

Reference context:
- `MAIL-FOUNDATION-PLAN.md`
- `mail-foundation-spec.md`
- `MAIL-FOUNDATION-M4-PHASE.md`
- `MAIL-FOUNDATION-M5-PHASE.md`
- `docs/adr/0001-rust-lambda-and-react-spa.md`
- `docs/adr/0002-public-authenticated-text-only-ui.md`
- `docs/adr/0003-shared-cognito-strong-auth.md`
- `../ahara/INTEGRATION.md`
- Current API routes in `backend/api/src/lib.rs`
- Current SPA scaffold in `frontend/src/`
- M2 storage model in `db/migrations/001_create_mail_model.sql`
- M5 inbound persistence/query fields in `backend/shared/src/inbound/`
- Terraform runtime config in `infrastructure/terraform/frontend.tf` and
  `infrastructure/terraform/locals.tf`

Exit gate:
- Focused Rust tests for shared mailbox service and API routes pass.
- Focused frontend tests for auth loading states, text-only rendering, inert
  dangerous links, auth-verdict display, read/unread, contact association,
  routing admin, and search pass.
- `make ci`
- Source scan confirms no compose/send/forward/feedback behavior, no active
  receipt/MX Terraform, and no HTML-rendering escape hatch.

## Step 1 - Confirm product, hostnames, and search scope  [DECISION]

Files:
- No code files. This is the M6 semantics decision before implementation.

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M6 has a phase-level `[DECISION]`: choose the MVP
  product/system name used in UI and DNS.
- `mail-foundation-spec.md` leaves system name/brand and search requirements
  open.
- The user previously said the likely final frontend should move toward bare
  `ahara.io`, with the existing Ahara portal moving to `cv.ahara.io`.

Change:
- Stop for user confirmation before editing code.
- Confirm:
  - Product/system display name used in browser title, app chrome, and runtime
    config.
  - M6 frontend hostname and API hostname to put in Terraform locals.
  - Whether M6 search is the recommended simple search: authenticated,
    case-insensitive substring search over accepted inbound `subject`,
    `from_address`, `from_display_name`, and `body_text`, with no ranking,
    highlighting, or full-text index in this phase.
- Recommended defaults if accepted:
  - Display name: `Ahara Mail`
  - Frontend hostname: `mail.ahara.io`
  - API hostname: `api.mail.ahara.io`
  - Search: simple accepted-inbound substring search as above

Verify:
- No automated verification. The executor records the confirmed values and
  then proceeds to step 2.

## Step 2 - Add shared mailbox read DTOs and safety helpers

Files:
- `backend/shared/src/mailbox.rs`
- `backend/shared/src/lib.rs`

Reference behavior:
- `mail-foundation-spec.md` defines message, recipient, attachment, thread,
  contact, auth-verdict, read/unread, and security-disposition fields.
- `mail-foundation-spec.md` requires plaintext-only reading, inert links by
  default, real `From` address display, auth verdict display, and untrusted
  attachment filename handling.
- M2 schema stores these fields in `messages`, `recipients`,
  `attachment_refs`, `threads`, and `contacts`.

Change:
- Add shared read-side DTOs for:
  `MailboxMessageSummary`, `MailboxMessageDetail`, `MailboxThreadDetail`,
  `MailboxRecipient`, `MailboxAttachment`, `MailboxQuery`, `MailboxSearchQuery`,
  `UpdateMessageStateRequest`, and `LinkMessageContactRequest`.
- Add helpers for:
  - body snippet creation from plaintext only,
  - attachment filename display sanitization,
  - accepted-message eligibility checks based on `direction`, `status`, and
    `security_disposition`,
  - auth/security DB-string parsing into stable JSON values.
- Do not add SQL, API routes, frontend code, raw MIME download, or outbound
  fields in this step.

Verify:
- Before the change:
  `! rg "MailboxMessageSummary|MailboxMessageDetail|sanitize_attachment_filename|MailboxSearchQuery" backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared mailbox_types`
- Red before: mailbox DTO/safety symbols do not exist. Green after: tests
  cover accepted-message eligibility, rejected/quarantined exclusion flags,
  plaintext snippet creation, path/control-character filename sanitization,
  auth/security value parsing, and request validation.

## Step 3 - Add shared mailbox read service with PostgreSQL and in-memory implementations  [depends on #2]

Files:
- `backend/shared/src/mailbox.rs`
- `backend/shared/tests/mailbox_model.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M6 requires mailbox queries, thread detail, message
  detail, contact links, message state, and search.
- `mail-foundation-spec.md` says quarantined/rejected mail never appears in
  normal mailbox queries.
- M2 indexes support accepted inbound unread and thread queries.
- M5 persists accepted/quarantined/rejected inbound messages with thread and
  contact links.

Change:
- Add `MailboxService` trait with methods:
  - `list_messages(query)`
  - `get_message(message_id)`
  - `get_thread(thread_id)`
  - `search_messages(query)`
  - `update_message_state(message_id, request)`
  - `link_message_contact(message_id, request)`
- Add `PgMailboxService` using parameterized SQL only.
- Add `InMemoryMailboxService` for API route and frontend-adjacent tests.
- `PgMailboxService` must filter normal list/detail/search/thread results to
  accepted inbound records only: `direction = 'inbound'`,
  `security_disposition = 'accepted'`, and `status = 'received'`.
- Read/unread and contact-link mutations must reject updates to non-accepted
  or non-inbound messages.
- Search uses the scope confirmed in step 1.
- Add a real PostgreSQL shape test using the existing container-network `psql`
  style if direct `sqlx` container connectivity is unavailable in this runner.
  The test must prove accepted mail is queryable and quarantined/rejected mail
  is excluded.

Verify:
- Before the change:
  `! rg "trait MailboxService|PgMailboxService|InMemoryMailboxService|list_messages" backend/shared/src backend/shared/tests`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p shared mailbox`
- Red before: service symbols do not exist. Green after: unit tests cover
  list/detail/thread/search/read-state/contact-link behavior, and the storage
  test proves normal mailbox queries exclude quarantined/rejected rows.

## Step 4 - Add authenticated mailbox read API routes  [depends on #3]

Files:
- `backend/api/src/lib.rs`
- `backend/api/Cargo.toml`

Reference behavior:
- ADR-0002 requires every non-health app route to be authenticated through
  shared Cognito/ALB JWT validation.
- M4 API already extracts validated bearer context and protects `/domains` and
  `/contacts`.
- `mail-foundation-spec.md` requires normal mailbox lists to exclude
  quarantined/rejected mail and surface real sender/auth/security metadata.

Change:
- Add `MailboxService` to `ApiState`.
- Wire `PgMailboxService` in `ApiState::from_env` and
  `InMemoryMailboxService` in `ApiState::for_tests`.
- Add authenticated routes:
  - `GET /mailbox/messages`
  - `GET /mailbox/messages/{message_id}`
  - `GET /mailbox/threads/{thread_id}`
  - `GET /mailbox/search?q=...`
- Return stable JSON DTOs from the shared mailbox module.
- Do not add compose/send/forward/download routes.

Verify:
- Before the change:
  `! rg "/mailbox/messages|/mailbox/threads|MailboxService" backend/api/src backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p api mailbox_read`
- Red before: mailbox routes/service state do not exist. Green after: route
  tests cover auth required, list accepted messages, detail shows plaintext
  body/auth verdict/real sender, thread detail excludes quarantined/rejected
  messages, search returns accepted messages only, and not-found behavior.

## Step 5 - Add authenticated message state and contact-link API routes  [depends on #3, #4]

Files:
- `backend/api/src/lib.rs`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M6 requires contact links and read/unread.
- `mail-foundation-spec.md` says `From` display is not identity; contact
  association is an explicit user workflow, not automatic trust in display
  names.
- ADR-0002 makes state-changing authenticated routes load-bearing.

Change:
- Add authenticated routes:
  - `PATCH /mailbox/messages/{message_id}/state`
  - `PATCH /mailbox/messages/{message_id}/contact`
- State route accepts only `{"is_read": boolean}`.
- Contact route accepts `{"contact_id": "<uuid-or-null>"}` and links/unlinks
  only accepted inbound messages.
- Do not auto-create contacts from sender display names.
- Do not change contact CRUD semantics from M4.

Verify:
- Before the change:
  `! rg "/mailbox/messages/\\{message_id\\}/state|LinkMessageContactRequest|UpdateMessageStateRequest" backend/api/src backend/shared/src`
- After the change:
  `cargo test --manifest-path backend/Cargo.toml -p api mailbox_state`
- Red before: routes do not exist. Green after: tests cover auth required,
  read/unread updates, contact link, contact unlink, rejected/quarantined
  mutation refusal, invalid contact id validation, and no display-name-based
  contact matching.

## Step 6 - Update Terraform/runtime config for confirmed UI identity  [depends on #1]

Files:
- `infrastructure/terraform/locals.tf`
- `infrastructure/terraform/frontend.tf`
- `infrastructure/terraform/outputs.tf`
- `frontend/src/config.ts`
- `frontend/src/vite-env.d.ts`

Reference behavior:
- `../ahara/INTEGRATION.md` uses website runtime config for SPA deployment and
  shared Cognito app-client wiring.
- ADR-0002 exposes the UI through the standard public platform web path.
- M6 step 1 confirms display name and hostnames.

Change:
- Update `local.frontend_hostname` and `local.api_hostname` with the confirmed
  M6 hostnames.
- Add runtime config keys needed by the SPA auth bootstrap:
  `appBaseUrl`, `productName`, and `cognitoDomain`.
- Keep `apiBaseUrl`, `cognitoUserPoolId`, and `cognitoClientId`.
- Update frontend config typing and Vite env typing to read the same values.
- Do not add MX records or activate receipt rules.

Verify:
- Before the change:
  `! rg "productName|appBaseUrl|cognitoDomain|VITE_COGNITO_DOMAIN" frontend/src infrastructure/terraform`
- After the change:
  `terraform fmt -check -recursive infrastructure/terraform/`
  and `cd frontend && pnpm exec tsc --noEmit`
- Red before: runtime keys do not exist. Green after: Terraform formatting and
  TypeScript config typing pass.

## Step 7 - Add frontend OIDC auth bootstrap  [depends on #6]

Files:
- `frontend/package.json`
- `frontend/pnpm-lock.yaml`
- `frontend/src/auth.ts`
- `frontend/src/auth.test.ts`
- `frontend/src/config.ts`

Reference behavior:
- ADR-0003 uses shared Cognito as identity source.
- `../ahara/INTEGRATION.md` says the frontend sends
  `Authorization: Bearer <access_token>` to the API and the ALB validates JWTs.
- ADR-0002 requires authenticated app access before mailbox content is shown.

Change:
- Add an OIDC/Cognito browser auth helper using Authorization Code + PKCE
  through the configured Cognito domain/app client.
- Provide a small local auth state contract:
  loading, signed-out, signed-in, and error states.
- Store tokens in browser storage only as needed for the SPA session; expose
  an `getAccessToken()` helper for API calls.
- Parse redirect/callback state without rendering mailbox data before auth is
  resolved.
- Keep the auth client injectable/testable.

Verify:
- Before the change:
  `! rg "AuthState|createAuthClient|getAccessToken|cognitoDomain" frontend/src`
- After the change:
  `cd frontend && pnpm exec vitest run src/auth.test.ts`
- Red before: auth module symbols do not exist. Green after: tests cover
  loading state, signed-out state, signed-in token availability, redirect
  callback handling, logout clearing state, and auth error rendering state.

## Step 8 - Add typed frontend API client  [depends on #4, #5, #7]

Files:
- `frontend/src/api.ts`
- `frontend/src/api.test.ts`
- `frontend/src/types.ts`

Reference behavior:
- M6 backend routes return shared mailbox/contact/domain DTOs.
- ADR-0002 requires authenticated API calls for application behavior.
- `mail-foundation-spec.md` requires plaintext body rendering and auth/security
  verdict display; the API client must preserve these fields without trying to
  interpret HTML.

Change:
- Add TypeScript DTOs matching M6 API JSON.
- Add API methods for:
  mailbox messages, message detail, thread detail, search, read/unread,
  contact link/unlink, contacts, domains, domain updates, address add/remove.
- Inject `Authorization: Bearer <token>` on all app API calls.
- Normalize API errors into a typed client error.
- Do not add compose/send/forward methods.

Verify:
- Before the change:
  `! rg "fetchMailboxMessages|fetchThreadDetail|updateMessageState|linkMessageContact|ApiClient" frontend/src`
- After the change:
  `cd frontend && pnpm exec vitest run src/api.test.ts`
- Red before: API client symbols do not exist. Green after: tests cover bearer
  header injection, message list/detail/search calls, state/contact mutation
  calls, domain/contact admin calls, and error normalization.

## Step 9 - Build authenticated app shell and mailbox list  [depends on #7, #8]

Files:
- `frontend/package.json`
- `frontend/pnpm-lock.yaml`
- `frontend/src/App.tsx`
- `frontend/src/App.test.tsx`
- `frontend/src/index.css`
- `frontend/src/mailbox.tsx`
- `frontend/src/mailbox.test.tsx`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M6 requires React SPA auth bootstrap, mailbox list,
  and authenticated loading states.
- Frontend design guidance for this operational tool favors dense,
  organized, work-focused UI rather than a landing page.
- ADR-0002 requires mailbox content behind auth.

Change:
- Add the authenticated app shell:
  loading, signed-out, auth-error, and signed-in states.
- Add the first signed-in screen as the mailbox list, not a landing page.
- Add compact navigation for mailbox, contacts/admin, and routing policy.
- Add mailbox list rows with sender address, display name as secondary text,
  subject/snippet, unread state, received time, auth verdict, attachment count,
  and selected-thread affordance.
- Use `lucide-react` icons for toolbar/action buttons.
- Keep stable dimensions for list rows/toolbars so content does not shift.

Verify:
- Before the change:
  `! rg "MailboxList|auth.*loading|signed-out|lucide-react" frontend/src frontend/package.json`
- After the change:
  `cd frontend && pnpm exec vitest run src/App.test.tsx src/mailbox.test.tsx`
- Red before: mailbox shell/list symbols do not exist. Green after: tests
  cover auth loading state, signed-out state, signed-in mailbox list render,
  unread marker, auth verdict display in list, and no mailbox content while
  auth is unresolved.

## Step 10 - Build thread and message detail with text-only rendering  [depends on #8, #9]

Files:
- `frontend/src/mailbox.tsx`
- `frontend/src/mailbox.test.tsx`
- `frontend/src/textRendering.ts`
- `frontend/src/textRendering.test.ts`
- `frontend/src/index.css`

Reference behavior:
- `mail-foundation-spec.md` requires text-only rendering, real sender address,
  auth verdicts, security disposition, inert links by default, and sanitized
  attachment metadata display.
- ADR-0002 makes no-HTML rendering a load-bearing boundary.
- M5 stored plaintext body text and attachment metadata only.

Change:
- Add thread detail and message detail views.
- Render `body_text` as plaintext using text nodes/pre-wrapped text only.
- Do not use `dangerouslySetInnerHTML`, `innerHTML`, or generated anchors for
  message-body URLs.
- Show real `from_address` prominently; show display name only as secondary
  untrusted display text.
- Show SPF/DKIM/DMARC/auth verdict, spam/virus values when present, and
  security disposition.
- Show attachment metadata with sanitized filename text, content type, and
  size; no raw MIME download action in M6.

Verify:
- Before the change:
  `! rg "MessageDetail|ThreadDetail|dangerouslySetInnerHTML|sanitizeAttachmentName|Auth verdict" frontend/src`
- After the change:
  `cd frontend && pnpm exec vitest run src/mailbox.test.tsx src/textRendering.test.ts`
- Red before: detail/text-rendering symbols do not exist. Green after: tests
  cover plaintext body display, `<script>` text not executing/rendering as
  HTML, `javascript:`/`data:` URL text staying inert, no anchor elements for
  message body links, real sender address display, auth-verdict display, and
  sanitized attachment filename display.

## Step 11 - Add read/unread and contact association UI  [depends on #5, #8, #10]

Files:
- `frontend/src/mailbox.tsx`
- `frontend/src/mailbox.test.tsx`
- `frontend/src/api.ts`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M6 requires contact association and read/unread.
- `mail-foundation-spec.md` treats display names as untrusted and contact
  association as explicit user action.
- Backend M6 state/contact routes require authentication.

Change:
- Add read/unread toggle controls in list/detail.
- Update local UI state after successful API mutation.
- Add contact association control using existing contacts from the API:
  link to a selected contact or unlink.
- Do not auto-create contacts or infer identity from sender display names.

Verify:
- Before the change:
  `! rg "mark.*read|unread|contact.*associate|linkMessageContact" frontend/src/mailbox.tsx frontend/src/api.ts`
- After the change:
  `cd frontend && pnpm exec vitest run src/mailbox.test.tsx`
- Red before: controls do not exist. Green after: tests cover marking read,
  marking unread, contact link, contact unlink, API error state, and no
  auto-association from display name.

## Step 12 - Add routing-policy admin UI using existing domain/address APIs  [depends on #8, #9]

Files:
- `frontend/src/routingAdmin.tsx`
- `frontend/src/routingAdmin.test.tsx`
- `frontend/src/App.tsx`
- `frontend/src/index.css`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M6 requires routing-policy admin.
- M4 API already provides authenticated domain/address config routes.
- `mail-foundation-spec.md` defines per-domain `allowlist` and `catchall`,
  plus active accepted addresses.

Change:
- Add an authenticated routing admin panel reachable from the app navigation.
- Show domains, routing policy, active state, accepted local parts, and address
  active state.
- Add controls to update policy/active state, add accepted local parts, and
  deactivate accepted local parts.
- Use segmented controls/toggles/inputs for policy and state.
- Do not add MX/routability controls.

Verify:
- Before the change:
  `! rg "RoutingAdmin|updateDomain|addAddress|deactivateAddress" frontend/src`
- After the change:
  `cd frontend && pnpm exec vitest run src/routingAdmin.test.tsx src/App.test.tsx`
- Red before: routing admin UI does not exist. Green after: tests cover domain
  list rendering, allowlist/catchall update, domain active toggle, address add,
  address deactivate, auth-required API error display, and no MX/routability
  controls.

## Step 13 - Add mailbox search UI  [depends on #1, #4, #8, #9]

Files:
- `frontend/src/mailbox.tsx`
- `frontend/src/mailbox.test.tsx`
- `frontend/src/api.ts`
- `frontend/src/index.css`

Reference behavior:
- `MAIL-FOUNDATION-PLAN.md` M6 requires search.
- Search scope is confirmed in step 1.
- `mail-foundation-spec.md` says quarantined/rejected mail never appears in
  normal mailbox queries.

Change:
- Add a search input/control in the mailbox toolbar.
- Call the M6 search API with the confirmed query semantics.
- Render search results using the same accepted-message list component.
- Show empty and error states without exposing raw query internals.
- Do not add full-text ranking/highlighting unless confirmed in step 1.

Verify:
- Before the change:
  `! rg "searchMessages|Search mailbox|mailbox-search" frontend/src`
- After the change:
  `cd frontend && pnpm exec vitest run src/mailbox.test.tsx`
- Red before: search UI does not exist. Green after: tests cover submitting a
  search, rendering accepted search results, empty results, rejected/quarantined
  exclusions as represented by API results, and preserving inert text rendering
  in search result snippets.

## Step 14 - Run the M6 exit gate  [depends on #13]

Files:
- No implementation files unless an earlier verification exposes a build-only
  wiring miss directly caused by M6.

Reference behavior:
- M6 exit in `MAIL-FOUNDATION-PLAN.md` requires `make ci` green and UI tests
  for authenticated loading states, text-only rendering, inert dangerous links,
  and auth-verdict display.
- M6 scope guard forbids compose/send/forward/feedback, MX/routability changes,
  and sender-HTML rendering.

Change:
- Run the focused M6 tests first, then the repository-wide gate.
- If a focused test fails, fix only the M6-scoped cause in the files named by
  the failed step.
- Do not continue into M7.

Verify:
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p shared mailbox`
- Run:
  `cargo test --manifest-path backend/Cargo.toml -p api mailbox`
- Run:
  `cd frontend && pnpm exec vitest run`
- Run:
  `make ci`
- Run:
  `! rg -n "dangerouslySetInnerHTML|innerHTML|outerHTML|href=.*body_text|href=.*bodyText" frontend/src`
- Run:
  `! rg -n "compose|reply|send_mail\\(|MailSender|forward|outbound_work|FeedbackPublisher|suppression|bounce|complaint" backend/api/src backend/shared/src frontend/src`
- Run:
  `! rg -n "type\\s*=\\s*\\\"MX\\\"|MX[[:space:]]+|^\\s*enabled\\s*=\\s*true|^\\s*active\\s*=\\s*true" infrastructure/terraform -g '*.tf'`
- The executor reports the actual output for the focused tests and `make ci`,
  then stops at the end of M6.
