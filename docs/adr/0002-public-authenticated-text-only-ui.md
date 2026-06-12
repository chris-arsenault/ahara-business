# 0002 - Public Authenticated Text-Only UI

- Status: Accepted
- Date: 2026-06-09

## Context

The product spec originally required a private-network-only UI. The platform's standard web path is public HTTPS through CloudFront and the shared ALB with Cognito. The mail client also strips HTML to plaintext at ingest and never renders sender-provided HTML, removing the primary email-content XSS class from the application surface.

## Decision

Expose the UI through the standard public platform web path, require Cognito authentication for app access, and keep mail rendering text-only.

## Alternatives considered

- **Strict VPN/private-only UI** - Stronger network boundary, but requires new platform ingress work and gives up the standard website/API deployment path.
- **TrueNAS reverse-proxy hosting** - Reuses existing WireGuard/reverse-proxy machinery, but adds deployment and data-path complexity while SES mail processing remains in AWS.

## Consequences

The public surface makes authentication, session lifetime, CSRF protection, rate limiting, and no-HTML rendering load-bearing. The plan keeps sender-controlled HTML out of the rendering path and routes all app behavior through authenticated API calls.
