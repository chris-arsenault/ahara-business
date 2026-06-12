# 0003 - Shared Cognito Strong Auth

- Status: Accepted
- Date: 2026-06-09

## Context

The mail foundation contains private business correspondence and becomes a base for CRM workflows. The platform already provides a shared Cognito user pool and app-client pattern, but its current Terraform does not enforce a strong-auth posture for this app's needs.

## Decision

Use shared Cognito and extend the platform configuration for required TOTP or passkey/WebAuthn support.

## Alternatives considered

- **Project-scoped second factor after Cognito** - Keeps the platform untouched, but duplicates security-sensitive auth code in the application.
- **Current Cognito password flow only** - Fits existing wiring but does not satisfy the mail foundation's strong-auth requirement.

## Consequences

The implementation includes an `ahara-infra` phase for Cognito strong-auth support before the app relies on the public authenticated UI. The frontend and API continue to use platform-issued Cognito tokens rather than a project-specific identity store.
