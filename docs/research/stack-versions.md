# Stack versions and TanStack Start on Rsbuild

Research for issue #2. Checked on **2026-11-18** against package registries
(crates.io, npm), official release pages and official docs. Versions move
fast; Renovate keeps them current after scaffolding. Re-check anything here
older than a few weeks before pinning it.

## Answer

**TanStack Start's SSR runs on Rsbuild with official, first-party support.**
`@tanstack/react-start` ships a `./plugin/rsbuild` export (since 1.167.43,
2026-04-24) and lists `@rsbuild/core ^2.0.5` as an optional peer. The docs say
Start "supports Vite or Rsbuild as the build tool" and show Rsbuild configs for
SSR, server entry, prerendering and RSC. The TanStack repo runs Playwright e2e
suites against Rsbuild builds. Nothing in the chosen stack needs Vite.

What you lose without Vite: Nitro (`nitro/vite`) and the vendor deploy plugins
(Netlify, Cloudflare) are Vite-only. Pranata runs in its own container behind
Caddy, so this does not matter. The Rsbuild build output is a fetch-style
server (`dist/server/index.js`) plus static assets (`dist/client`), which you
serve with `srvx` (the documented way) or `Bun.serve`.

One stack change is needed: **TypeScript 7.0 has no programmatic API**, so
tools that embed the compiler must stay on 6.0 (see Compatibility notes).

## Versions

### Backend (Rust)

| Tool                | Latest stable                       | Source                                                     | Checked    |
| ------------------- | ----------------------------------- | ---------------------------------------------------------- | ---------- |
| Rust toolchain      | 1.100.0                             | https://static.rust-lang.org/dist/channel-rust-stable.toml | 2026-11-18 |
| axum                | 0.8.9                               | https://crates.io/crates/axum                              | 2026-11-18 |
| sqlx                | 0.9.1 (released 2026-11-18)         | https://crates.io/crates/sqlx                              | 2026-11-18 |
| uuid (feature `v7`) | 1.29.3                              | https://crates.io/crates/uuid                              | 2026-11-18 |
| argon2              | 0.6.0                               | https://crates.io/crates/argon2                            | 2026-11-18 |
| utoipa              | 6.0.0                               | https://crates.io/crates/utoipa                            | 2026-11-18 |
| utoipa-axum         | 0.3.0                               | https://crates.io/crates/utoipa-axum                       | 2026-11-18 |
| typst               | 0.15.1                              | https://crates.io/crates/typst                             | 2026-11-18 |
| typst-pdf           | 0.15.1                              | https://crates.io/crates/typst-pdf                         | 2026-11-18 |
| calamine            | 0.38.0                              | https://crates.io/crates/calamine                          | 2026-11-18 |
| PostgreSQL          | 18.7 (19 is at RC2, not stable yet) | https://www.postgresql.org/support/versioning/             | 2026-11-18 |

### Frontend (TypeScript)

| Tool                                   | Latest stable                                     | Source                                               | Checked    |
| -------------------------------------- | ------------------------------------------------- | ---------------------------------------------------- | ---------- |
| Bun                                    | 1.4.4                                             | https://github.com/oven-sh/bun/releases              | 2026-11-18 |
| Node.js (LTS, build tooling)           | 26.13.1 "Lithium"                                 | https://nodejs.org/dist/index.json                   | 2026-11-18 |
| React / react-dom                      | 19.3.1                                            | https://www.npmjs.com/package/react                  | 2026-11-18 |
| @tanstack/react-start                  | 1.169.5                                           | https://www.npmjs.com/package/@tanstack/react-start  | 2026-11-18 |
| @tanstack/react-router                 | 1.170.46                                          | https://www.npmjs.com/package/@tanstack/react-router | 2026-11-18 |
| tailwindcss / @tailwindcss/postcss     | 4.3.5                                             | https://www.npmjs.com/package/tailwindcss            | 2026-11-18 |
| shadcn (CLI)                           | 4.23.0                                            | https://www.npmjs.com/package/shadcn                 | 2026-11-18 |
| @base-ui/react                         | 1.9.0                                             | https://www.npmjs.com/package/@base-ui/react         | 2026-11-18 |
| lucide-react                           | 1.75.0                                            | https://www.npmjs.com/package/lucide-react           | 2026-11-18 |
| @tiptap/react / @tiptap/starter-kit    | 3.34.0                                            | https://www.npmjs.com/package/@tiptap/react          | 2026-11-18 |
| orval                                  | 8.48.0                                            | https://www.npmjs.com/package/orval                  | 2026-11-18 |
| typescript                             | 7.0.2 (6.0 line: `@typescript/typescript6` 6.0.2) | https://www.npmjs.com/package/typescript             | 2026-11-18 |
| @rsbuild/core                          | 2.3.3                                             | https://www.npmjs.com/package/@rsbuild/core          | 2026-11-18 |
| @rsbuild/plugin-react                  | 2.1.1                                             | https://www.npmjs.com/package/@rsbuild/plugin-react  | 2026-11-18 |
| @biomejs/biome                         | 2.6.8                                             | https://www.npmjs.com/package/@biomejs/biome         | 2026-11-18 |
| @rstest/core / @rstest/adapter-rsbuild | 0.12.6                                            | https://github.com/web-infra-dev/rstest/releases     | 2026-11-18 |

### Tooling and operations

| Tool                                  | Latest stable                          | Source                                            | Checked    |
| ------------------------------------- | -------------------------------------- | ------------------------------------------------- | ---------- |
| lefthook                              | 2.2.2                                  | https://github.com/evilmartians/lefthook/releases | 2026-11-18 |
| @commitlint/cli / config-conventional | 21.2.4                                 | https://www.npmjs.com/package/@commitlint/cli     | 2026-11-18 |
| Renovate                              | 44.252.2 (only matters if self-hosted) | https://www.npmjs.com/package/renovate            | 2026-11-18 |
| just                                  | 1.59.0                                 | https://github.com/casey/just/releases            | 2026-11-18 |
| Podman                                | 6.2.0                                  | https://github.com/containers/podman/releases     | 2026-11-18 |
| Caddy                                 | 2.11.9                                 | https://github.com/caddyserver/caddy/releases     | 2026-11-18 |

## TanStack Start on Rsbuild: evidence

- **Package exports.** `@tanstack/react-start@1.169.5` exports
  `./plugin/rsbuild`, `./rsbuild/ssr-decode` and `./rsbuild/browser-decode`.
  Its peers are `vite >=8.0.0` (optional) and `@rsbuild/core ^2.0.5`
  (optional). The first version with the Rsbuild export was 1.167.43
  (2026-04-24). Source: https://registry.npmjs.org/@tanstack/react-start
- **Docs.** Build from scratch: "TanStack Start is powered by TanStack Router
  and supports Vite or Rsbuild as the build tool". It installs
  `@rsbuild/core @rsbuild/plugin-react` and uses `rsbuild dev` / `rsbuild build`.
  https://tanstack.com/start/latest/docs/framework/react/build-from-scratch
- **Hosting docs.** "TanStack Start supports Vite and Rsbuild". "An Rsbuild
  production build emits client assets in dist/client and a server bundle in
  dist/server/index…", exporting `{ fetch(request): Response }`. The documented
  start script is `srvx --prod -s ../client dist/server/index.js`. The Nitro
  path is documented for Vite only.
  https://tanstack.com/start/latest/docs/framework/react/guide/hosting
- **Config surface.** The server entry, prerender and RSC guides each give an
  `rsbuild.config.ts` using `tanstackStart()` from
  `@tanstack/react-start/plugin/rsbuild` together with `pluginReact()`.
  https://tanstack.com/start/latest/docs/framework/react/guide/server-entry-point
- **CI coverage.** The TanStack/router repo has Rsbuild e2e apps
  (`e2e/react-start/rsbuild`, `custom-server-rsbuild`, `rsc-rsbuild`, plus
  Rsbuild configs in `basic`, `css-modules`, `hmr`, `import-protection` and
  others). https://github.com/TanStack/router/tree/main/e2e/react-start
- **Open Rsbuild-specific issues (2):**
  [#8545](https://github.com/TanStack/router/issues/8545) server-only routes
  are not pruned from the client route tree (a bundle-size and leak hygiene
  concern; keep secrets out of route files anyway).
  [#7827](https://github.com/TanStack/router/issues/7827) `tanstackStart()`
  overrides the client `output.assetPrefix` (matters only when serving assets
  from a CDN prefix).

### What breaks or differs without Vite

- No Nitro presets and no `@netlify/vite-plugin-tanstack-start` or Cloudflare
  plugin. Not needed: each Installation is a self-hosted container.
- Vite-only community plugins don't apply. Use Rsbuild/Rspack plugins
  instead. Tailwind v4 goes through `@tailwindcss/postcss`, which TanStack's
  own Rsbuild e2e app does (`postcss.config.mjs`).
- The Bun-specific production server in the hosting guide is written for
  React 19, which we use anyway.

## Compatibility notes

- **TypeScript 7.0 ships without an API.** The 7.0 announcement: "it does not
  ship with an API". Tools that need programmatic compiler access use the
  `@typescript/typescript6` compatibility package side by side. A new API is
  planned for 7.1 (beta 2026-10-20).
  https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/
  For Pranata: use `tsc` from TS 7 for `typecheck` in CI and lefthook. Rsbuild
  and Rstest transpile with SWC, and Biome has its own parser, so neither
  needs the TS API. Don't add `@rsbuild/plugin-type-check` (it embeds the
  compiler); run `tsc --noEmit` as a separate step. shadcn bundles its own
  compiler through `ts-morph`. Orval and the TanStack packages declare no
  dependency on `typescript`.
- **utoipa-axum 0.3.0 and axum 0.8.9: compatible.** utoipa-axum requires
  `axum ^0.8.4` and `utoipa ^6.0.0`. The same holds for `utoipa-scalar` 0.4.0.
  utoipa 6 has `axum_extras`, `uuid`, `chrono` and `time` features. There is
  no axum 0.9 release yet; the 0.8 line is current.
  https://crates.io/api/v1/crates/utoipa-axum/0.3.0/dependencies
- **sqlx 0.9.1 came out today** (0.9.0 on 2026-05-21) and needs Rust 1.94.
  It has the `postgres`, `runtime-tokio`, `tls-rustls`, `uuid`, `chrono`,
  `time`, `migrate` and `macros` features. Pin `0.9` and let Renovate take
  patches.
- **UUIDv7.** PostgreSQL 18 has a native `uuidv7()` function
  (https://www.postgresql.org/docs/18/functions-uuid.html), and `uuid` 1.29.3
  has the `v7` feature. IDs can be generated in either place. Pick one in the
  data-model ticket.
- **MSRV.** The highest minimum Rust version among the picks is sqlx (1.94),
  then typst (1.92). Toolchain 1.100.0 covers all of them. Pin it in
  `rust-toolchain.toml`.
- **shadcn Base UI on Tailwind v4: supported, and now the default.** Since the
  July 2026 changelog, "Base UI is the default component library in
  shadcn/ui". `shadcn init` picks Base UI unless you pass `-b radix`
  (https://ui.shadcn.com/docs/changelog/2026-07-base-ui-default). CLI v4 added
  `--base` (https://ui.shadcn.com/docs/changelog/2026-03-cli-v4). The
  generated CSS targets Tailwind v4 (`@import "tailwindcss"`,
  `@theme inline`, `tw-animate-css`)
  (https://ui.shadcn.com/docs/installation/manual,
  https://ui.shadcn.com/docs/tailwind-v4).
- **Orval TanStack Query output.** `client: 'react-query'` generates TanStack
  Query v5 hooks: `useQuery`, `useSuspenseQuery`, `useMutation`, infinite
  queries, prefetch, invalidation and exported query keys. A custom fetch
  `mutator` lets you route calls through one place for cookies and errors.
  https://github.com/orval-labs/orval/blob/master/docs/content/docs/reference/configuration/output.mdx . Orval 8.48 needs
  Node >= 22.18.
- **Rstest: pre-1.0 (0.12.6) but active.** It has Jest-compatible APIs and an
  official `@rstest/adapter-rsbuild` (`withRsbuildConfig()`) that reuses the
  Rsbuild aliases and plugins. DOM tests run on `happy-dom` or `jsdom`.
  https://github.com/web-infra-dev/rstest/blob/main/website/docs/en/guide/framework/react.mdx . Expect breaking changes in minor
  releases until 1.0. Group the `@rstest/*` updates in Renovate.
- **Runtimes.** Rsbuild 2 runs on Node 20.19+/22.12+, Bun or Deno
  (https://github.com/web-infra-dev/rsbuild/blob/main/website/docs/en/guide/start/quick-start.mdx). TanStack Start and Orval
  declare Node >= 22.12 and >= 22.18. Use Bun as the package manager and
  script runner. Keep Node 26 LTS on CI and in the build image as the runtime
  that every engine field declares.
- **Tiptap 3.34** accepts React 17–19 as a peer.

## Recommended additions

| Addition                                                     | Version                  | Why                                                                                                 |
| ------------------------------------------------------------ | ------------------------ | --------------------------------------------------------------------------------------------------- |
| @tanstack/react-query                                        | 5.108.0                  | Orval's `react-query` client generates hooks for it.                                                |
| @tanstack/react-router-ssr-query                             | 1.167.3                  | Official glue that dehydrates and hydrates Query cache through Start SSR.                           |
| srvx                                                         | 1.0.8                    | Documented production server for the Rsbuild `dist/server` fetch entry.                             |
| @rstest/adapter-rsbuild + happy-dom + @testing-library/react | 0.12.6 / 21.0.4 / 16.3.3 | Component tests that reuse the Rsbuild config.                                                      |
| @playwright/test                                             | 1.65.1                   | E2E runner, the same one TanStack uses for its own Start e2e suites.                                |
| @typescript/typescript6 (only if a tool needs it)            | 6.0.2                    | Official side-by-side TS 6 API while TS 7 has none.                                                 |
| tokio                                                        | 1.54.1                   | Async runtime for axum and sqlx.                                                                    |
| tower-http                                                   | 0.7.1                    | Trace, request-id, compression, CORS and timeout middleware (needs `tower ^0.5`, same as axum 0.8). |
| tracing + tracing-subscriber                                 | 0.1.44 / 0.3.23          | Structured logs and spans across axum, sqlx and tower-http.                                         |
| thiserror                                                    | 2.0.21                   | Typed domain/API errors that map to HTTP responses.                                                 |
| anyhow                                                       | 1.0.104                  | Error context in binaries, CLI tasks and tests (not in the domain layer).                           |
| serde + serde_json                                           | 1.0.229 / 1.0.154        | Serialization, needed by axum, utoipa and sqlx JSON.                                                |
| utoipa-scalar                                                | 0.4.0                    | API reference UI served from the binary (axum ^0.8.4, utoipa ^6).                                   |
| chrono                                                       | 0.4.45                   | One date/time type that calamine (`dates`), sqlx and utoipa all support.                            |
| garde                                                        | 0.23.0                   | Declarative input validation at the API boundary (`validator` 0.21.0 is the alternative).           |
| tw-animate-css                                               | 1.4.0                    | Replaces `tailwindcss-animate`, as the shadcn Tailwind v4 setup requires.                           |

## Open points

- Whether `@tanstack/react-start/plugin/rsbuild` runs fully under the Bun
  runtime (`bun --bun rsbuild …`) is not documented. Rsbuild supports Bun, but
  TanStack's Rsbuild e2e suites use Node. Verify in the scaffold ticket, and
  fall back to Node 26 for build and serve if needed.
- PostgreSQL 19 is at RC2. If it goes GA before the first release, start on
  19 instead of 18.
- sqlx 0.9.1 is hours old. Watch for a quick 0.9.2.
