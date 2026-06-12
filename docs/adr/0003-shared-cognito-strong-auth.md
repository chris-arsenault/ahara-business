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

Shared Cognito remains the identity source for the frontend, ALB, and API. The app uses platform-issued Cognito tokens rather than a project-specific identity store.
