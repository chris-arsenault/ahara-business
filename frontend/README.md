# Frontend

Vite React/TypeScript SPA for Ahara Mail.

The app reads `window.__APP_CONFIG__` from the platform website runtime config,
authenticates through the shared Cognito app client, and calls the project API
with Cognito access tokens. The primary workspace includes mailbox reads,
thread detail, read/unread state, search, contact linking, text-only
compose/reply, sent mail, routing admin, and address-scoped forwarding rules.

Mail content rendering follows ADR-0002: stored plaintext only, no
sender-provided HTML rendering, inert links, and untrusted display handling for
sender names and attachment metadata.

## Verification

```bash
pnpm install --frozen-lockfile
pnpm exec eslint .
pnpm exec tsc --noEmit
pnpm exec vitest run --coverage
pnpm run build
```
