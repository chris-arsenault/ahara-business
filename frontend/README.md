# Frontend

Vite React/TypeScript SPA scaffold for the public authenticated app surface.

The app reads `window.__APP_CONFIG__` from the platform website runtime config
and renders a neutral authenticated-app shell. It does not render mail content
or call project API routes in M0.

Mail UI work must keep ADR-0002 intact: stored plaintext only, no
sender-provided HTML rendering, and sender-controlled names, links, and filenames
treated as untrusted display data.

## Verification

```bash
pnpm install --frozen-lockfile
pnpm exec eslint .
pnpm exec tsc --noEmit
pnpm exec vitest run --coverage
pnpm run build
```
