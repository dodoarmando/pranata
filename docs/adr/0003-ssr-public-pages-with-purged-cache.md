# Server-rendered public pages behind a purged HTML cache

The public site is rendered by TanStack Start on Bun from the Rust API, and rendered HTML is kept in a bounded in-memory cache that the API purges after every change affecting public pages. Admin routes are client-only (`ssr: false`). A k6 load test in CI (300 concurrent visitors, p95 under 1 s, within 2 vCPU / 2 GB) fails the build if this stops holding.

The reason is the product's main promise: OpenSID-style slowdowns when a link spreads through WhatsApp groups are what villages complain about. React SSR costs CPU per request, so the cache is not an optimisation but part of the design.

## Considered Options

- **Rust renders public pages with templates; admin as a React SPA**: one server runtime and the fastest under load, but two ways to write UI, and public pages could not reuse React/shadcn components. This stays the fallback for public pages if the load test cannot be met; the API contract would not change.
- **Drop Rust and run everything in TanStack Start**: one runtime, but contradicts the chosen backend and the letter/PDF work planned for R3.
- **Caddy HTTP cache plugin**: would need a custom Caddy build to maintain.

## Consequences

Production runs two runtimes (Rust and Bun). Every write path that changes public content must emit a purge signal, which integration tests check. Signed-in responses are never cached.
