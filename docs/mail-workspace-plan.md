# Mail workspace and attachment transport

Outcome: send up to 15 MB of attachments without sending file bytes through the
application WAF, and replace the mailbox presentation with an efficient
conversation workspace. Implementation and validation are complete locally.
The user has authorized committing and pushing these changes on the current
`main` branch; deployment must be verified separately.

User requirements: direct S3 attachment transport, a 15 MB limit, threaded sent
and received messages, compact scan details, and a structural design overhaul
covering navigation, lists, reading and replying. Preserve recipient routing,
existing mail, text-only rendering, authentication and private storage.

Implementation decisions: reuse the private mail bucket with a separate attachment
prefix. Authenticated API requests grant uploads and resolve opaque upload IDs;
the browser transfers bytes directly to S3. Pin the uploaded object version before
queueing so an outstanding upload URL cannot alter queued mail. Keep existing
database payloads readable. Switch sending to SES v2 because its encoded-message
limit accommodates a 15 MB attachment budget. Interpret the budget as 15 MiB
combined across at most ten files, matching the existing aggregate/count policy.
No upload queue, recovery worker or WAF exception is needed.

Sources and reuse: `backend/shared/src/outbound.rs` owns validation, durable queue
and MIME assembly; `attachments.rs` extracts accepted received attachments;
`raw_mail_store.rs` establishes existing storage conventions. API routes authorize
mailbox users. `frontend/src/mailbox.tsx` and the conversation/composer components
are the consumers. `infrastructure/terraform/mail_storage.tf` owns the existing
encrypted, versioned bucket. The platform contract remains `../ahara/INTEGRATION.md`.

Milestones and Sulion mapping:

1. S3 attachments and SES v2: direct uploads/downloads, metadata-only API messages,
   pinned durable references, legacy compatibility, aggregate limit and failure
   coverage. Parent phase `d67e210b-7e7c-457e-bf61-3a2a0499458d`.
2. Workspace overhaul [depends on M1]: navigation, grouped conversation list,
   full-height reader, inline reply and a coherent compose workflow; compact scan
   disclosure, responsive layout and usable file progress/removal. Parent phase
   `ed5b92ec-5175-49db-b05d-587c7f121d70`.
3. Complete-flow checks [depends on M1, M2]: relevant behavioral tests, desktop and
   phone browser checks, production build, and `make ci`. Parent phase
   `29df5961-d87e-4435-a845-777cfffee872`.

Root plan: `e78b65f7-4359-4c59-ac4c-5fb171610ffc`.

## M1 — Direct private attachment transport

Expansion `ecaa4221-fccb-497a-b3e0-988c235c9d55`; steps
`e78f6020-cdcb-4660-8488-a3c969400632`,
`b4fdf95e-18e4-4c52-8630-3f4f67e7634e`,
`c4a9fe7f-eec3-4236-ab81-f1fdcf0203af`.

Acceptance: selecting files uploads their bytes directly to private S3; compose
and reply contain only upload IDs. Queueing verifies ownership, size and the
stored object, and retains a version-pinned reference for worker retries. File
downloads use short-lived S3 URLs. Existing attachment payloads remain usable.
A combined 15 MiB selection sends through SES v2; oversize input fails clearly.

Execution steps (initially pending):

1. Storage contract and API: add upload records and version-pinned references,
   presigned upload/download storage operations and authenticated metadata routes.
   Integrate queue validation, MIME reads and legacy attachment downloads. Verify
   ownership, missing/tampered objects, size limits and durable queue behavior.
2. Browser transport and delivery [depends on #1]: direct PUT with no application
   token, upload status/removal in compose and reply, metadata-only submission,
   direct downloads; SES v2 raw sending. Verify API and browser transfer boundaries
   plus encoded message size at the attachment limit.
3. Infrastructure and contract [depends on #1]: exact-origin bucket CORS and scoped
   attachment IAM; update the product specification and operational documentation.
   Run applicable Terraform checks and the shared final gates.

M1 state: implemented and locally validated. API tests reject inline bytes and
return signed download URLs. Real PostgreSQL tests cover upload ownership,
missing/mismatched objects, aggregate size, immutable queue references, retries
and legacy payloads. Local HTTP tests exercise the S3 adapter and presigner.
`make ci`, Terraform validation and the frontend production build passed. The
original production rejection reason remains unverified.

Live AWS checks are unavailable: the prescribed broker returned exit 66,
403 Forbidden, `no secret is unlocked for this terminal`; do not retry or use
alternate credentials until the user restores that path. The existing CI/CD
workflow runs on a push to `main`; deployment status must come from its results.

## M2 — Complete workspace interaction review

Expansion `13de6465-c57f-4eb6-8d31-889f1c187668`; steps
`d0257414-2d21-493b-a3f6-7b89dc682861` and
`d20fa70a-cc97-430d-a8b3-0f1a01fc42f4`.

Acceptance: navigation separates daily workspace tools from management; desktop
uses a conversation list and independent reader, while phone layouts switch
between list and reader. Sent and received replies share chronological threads;
scan metadata is compact. Compose focuses its editor, preserves a closed draft,
supports Cc/Bcc, and shares upload/removal behavior with inline reply.

Existing implementation from the interim mailbox work and M1 is retained. The
remaining steps reconcile it against the user's overhaul requirement:

1. Review and finalize conversation/navigation behavior in
   `mailbox.tsx`, `conversation.tsx`, `mailboxPresentation.ts` and
   `workspaceNavigation.tsx`. Check grouped Inbox/Sent lists, read-state updates,
   legacy sent-mail attachments, responsive navigation and compact security.
2. Review desktop and phone workflows [depends on #1] using a temporary local
   browser harness with synthetic mail. Inspect screenshots, exercise a real
   browser checksum and raw PUT at the attachment limit, reopen compose drafts,
   and stop the test server afterward. No hosted presentation or user data.

M2 state: workflow review completed. Inbox and Sent group conversations; legacy
standalone sent messages use the same reader and retain attachment access.
Recipient metadata retains To/Cc/Bcc. Obsolete compose grid rules were removed.
Desktop and phone screenshots were inspected locally. Real Chromium transferred
15 MiB and 6 KiB respectively directly to a synthetic S3 endpoint, checked actual
SHA-256 and absence of the application token, queued a reference-only reply,
preserved a closed compose draft and exercised phone navigation. Temporary
servers and browser processes stopped after the runs. No live mail was sent.

M3 state: `make ci`, production frontend build and `terraform validate` passed.
Final frontend checks cover the subsequent recipient/legacy-reader refinements:
all 96 tests passed, TypeScript and production build passed, and lint passed with
ten nonblocking warnings. Formatting, file-length and whitespace checks passed.
Rust 1.99 Clippy also passed for all workspace targets in release mode.
No AWS deployment or live S3 CORS/IAM test has been verified. Publication is now
authorized; live checks through this terminal still require restoration of the
previously denied AWS credential path.
