# 0001 - Rust Lambda and React SPA

- Status: Accepted
- Date: 2026-06-09

## Context

The mail foundation needs an authenticated web UI, multiple AWS event handlers, shared PostgreSQL access, and integration with the existing Ahara platform modules. Adjacent Ahara projects use Rust Lambdas and TypeScript/React frontends, and the platform modules are built around Lambda binaries plus static website deployment.

## Decision

Build the backend as a Rust Lambda workspace and the web client as a TypeScript/React SPA.

## Alternatives considered

- **Rust backend with server-rendered UI** - Simpler browser security surface and a natural fit for text-only mail, but less aligned with the platform website/runtime-config conventions.
- **TrueNAS-hosted app** - Useful for private network placement, but it splits the mail pipeline across AWS and TrueNAS and does not match the SES/Lambda-first product shape.

## Consequences

The implementation uses `cargo lambda` for backend artifacts and `pnpm`/Vite conventions for the frontend. Shared logic belongs in Rust library crates so parsing, policy, and SQL behavior remain testable outside Lambda handlers.
