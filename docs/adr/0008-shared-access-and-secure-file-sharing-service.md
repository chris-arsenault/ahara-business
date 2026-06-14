# 0008 - Shared Access And Secure File Sharing Service

- Status: Accepted
- Date: 2026-06-12

## Context

Post-MVP Ahara Business work expands beyond mail into an operator hub for
calendar, booking, money, mentee-facing workflows, and cross-app operational
tools. Tsonu Music also needs limited-audience sharing for pre-release files,
such as granting a mastering engineer access without publishing the files.
Ahara Portal, Ahara Business, Tsonu Music, and future apps should operate as
facets of one Ahara platform from the internal operator view.

These requirements all need external principals, object-level authorization,
revocation, and access audit. Implementing those concepts separately in each
repo would create inconsistent security behavior and make cross-app user
management brittle.

## Decision

Create `ahara-access` as a shared access and secure file-sharing backend
service. It is not an additional user-facing app or login surface. Product apps
keep ownership of their domain objects and reference those objects through
stable resource identifiers. The shared service owns principals, audiences,
grants, assets, and access events.

Secure file delivery uses private storage and short-lived delivery credentials,
such as CloudFront signed URLs. External recipients authenticate through shared
Cognito. Product APIs and public catalog APIs do not expose private bucket
names, object keys, S3 version IDs, upload ETags, or long-lived direct S3
access.

## Alternatives considered

- **Implement grants inside Ahara Business** - Fast for mentee workflows, but
  does not serve Tsonu Music or future apps and would make the mail repo a
  platform authorization owner.
- **Implement file sharing inside Tsonu Music** - Fits the first mastering
  engineer use case, but duplicates identity and audit concepts needed by
  Business Hub.
- **Use unlisted public links** - Simple to share, but access is public by
  obscurity and does not provide reliable identity, revocation, or audit.
- **Expose S3 presigned URLs directly from each app** - Useful for operator
  uploads, but weak as a shared external access model because policy,
  revocation, and audit remain fragmented.

## Consequences

Business Hub features in this repo can design for external visibility from the
beginning without shipping public-facing workflows first. Tsonu Music can share
pre-release files without weakening the public catalog boundary. Ahara Portal
remains the public recruiter-facing site, while platform app-authorization
administration and other operator-only workflows live in Ahara Business. The
shared access service owns object-level grants and audit behavior.

The platform gains another service boundary. Product apps must define stable
resource identifiers and call the shared service for grants and file-sharing
flows instead of reading another app's private storage directly.
