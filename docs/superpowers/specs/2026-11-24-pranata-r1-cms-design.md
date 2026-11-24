# Pranata R1 (CMS) — Design

- Date: 2026-11-24
- Status: Draft, awaiting approval
- Scope: R0 (walking skeleton) and R1 (CMS). R2–R4 are outlined only so R1 does not block them.
- Inputs: `.prototype/` (public site, login, admin), the archived attempt at `~/Projects/archive/pranata` (v0.2.0), and the brainstorming conversation of 2026-11-24.
- Companion documents: `CONTEXT.md` (glossary), `docs/adr/` (architecture decisions).

## 1. Intent

Pranata is a village website and administration product for Indonesian villages (desa), released under AGPLv3. Each village runs its own Installation. The first village is Desa Sidomukti (Kec. Pangkalan Kuras, Kab. Pelalawan, Riau), where the maintainer works as a village official; the next ones are the other villages of the same district and regency, then any village.

Today Sidomukti uses WordPress.com for its site, Prodeskel for population data, and Siskeudes for finance. The maintainer evaluated OpenSID, the dominant open-source village system, and rejected it: it becomes slow when many people open it at once, its user experience is poor, and it looks dated.

**What makes Pranata worth choosing is speed and feel, not feature count:** a public site that stays fast on a cheap phone when a link spreads through WhatsApp groups, and an admin that village staff can use without long training.

### Why this is a rebuild

The archived attempt used the same prototype and almost the same stack and reached v0.2.0. It was abandoned because of process, not technology: decisions agreed in the up-front design ("grilling") were never turned into issues, so they were silently dropped. Example: shadcn was decided but only applied after a corrective issue (#77), when dozens of feature pages already existed without it.

This design therefore treats **traceability of decisions** as a first-class requirement (section 6, D-42): every decision below has an ID, every ID must be implemented by at least one plan task and verified by a named check, and CI fails when a decision has no task.

The archive is input, not inheritance. Its glossary and ADRs were reviewed; what survives is restated here and in `CONTEXT.md`.

## 2. Goals, non-goals, anti-goals

### Goals (R1)

1. Sidomukti goes live on Pranata and stops using WordPress.
2. Any village can be set up without code changes: nothing village-specific lives in code.
3. A stranger can install one village on a fresh VPS by following the repository's installation guide alone.
4. The public site stays fast under load: p95 under 1 s with 300 concurrent visitors on a 2 vCPU / 2 GB VPS.
5. Every decision in this document is implemented and checked (no repeat of the archive's dropped decisions).

### Non-goals (R1)

- Population records, letters, online letter requests, complaints (R2–R4).
- Importing content from WordPress. Old content stays where it is; the village re-enters what it wants. Pranata gets a new domain.
- Email sending of any kind.
- Multi-tenancy or a shared hosting mode.
- Scheduled publishing, comments, newsletters.

### Anti-goals (failure even if everything "works")

- Pages that slow down when many residents open them at once.
- An admin menu that keeps growing until nobody can find anything (the OpenSID feeling).
- A look that feels like an old government application.
- A general-purpose page builder / WordPress clone. Pranata has only the content types a village needs.
- A decision from this document that is not visible in the product.

## 3. Constraints

- **Users and devices.** Residents mostly use phones. Village staff write news sometimes from a phone (often with weak signal) and do everything else on an office PC.
- **Hosting.** One VPS per village (ADR 0001), paid by the village, operated by the maintainer as a paid installation service, or self-hosted by anyone. Target size: 2 GB RAM, 1–2 vCPU. Everything (API, SSR server, PostgreSQL, Caddy, backups) must fit.
- **Language.** UI in Indonesian. Code, identifiers, commits, branches, issues, PRs, ADRs and documentation in English.
- **Law.** Personal data falls under UU 27/2022 (PDP). R1 stores almost no personal data (staff accounts, public names of officials), which is why encryption at rest is deferred (ADR 0005). Population data in R2 reopens this.
- **Versions.** Always the latest stable version of each technology, checked against the official registry at the time of use (section 8).
- **Process.** Atomic Conventional Commits, DCO sign-off, protected `main`, all changes through PRs with green CI.

## 4. Approaches considered

The decision that shapes the rest is who renders the public pages, because it decides how the site behaves under load.

| | A. Rust API + TanStack Start for public and admin | B. Rust renders public pages, admin is a React SPA |
|---|---|---|
| How | Public routes are server-rendered by TanStack Start (Bun) from the Rust API, behind an HTML cache purged on publish. Admin routes run client-side only (`ssr: false`). | The Rust API renders public pages with HTML templates and a little vanilla JS; the admin is a static React SPA. |
| For | One UI technology (React, Tailwind, shadcn) everywhere; easiest for open-source contributors; the generated API client is used everywhere. | One server runtime; fastest under load even without a cache; Bun only at build time. |
| Against | Two runtimes in production; React SSR costs CPU per request, so the cache is mandatory. | Two ways to write UI; public pages cannot reuse React/shadcn components; changing the public look means touching Rust templates. |

A third option, dropping Rust and running everything in TanStack Start, was not considered further: it contradicts the chosen backend and the letter/PDF work planned for R3.

**Chosen: A** (maintainer's choice), with two conditions that are decisions of their own: an HTML cache purged on publish (D-12) and a load test that fails CI (D-07). If the load test cannot be met, the fallback is B for public pages only; the API contract stays the same, so the move is contained. See ADR 0003.

## 5. Architecture overview

```
Resident's phone ─► Caddy (TLS, static media) ─► web: TanStack Start on Bun
                                                   ├─ public routes: SSR + in-memory HTML cache
                                                   └─ admin routes: client-only SPA
                                                          │  generated client (OpenAPI)
                                                          ▼
                                                 api: Rust / Axum ── PostgreSQL
                                                   ├─ background jobs (Postgres queue)
                                                   └─ media volume (originals + WebP variants)
```

- `apps/api` owns all data and every business rule. The web app never talks to the database.
- `apps/web` renders. For public pages it calls the API from route loaders during SSR; for admin pages it calls the API from the browser.
- The API tells the web server to purge cached HTML after any change that affects public pages.
- `infra/` provisions one Installation per VPS with Ansible: Debian 13, rootless Podman with Quadlet units for `api`, `web`, `postgres`, `caddy`, plus backup timers.

### Repository layout (proposed)

```
apps/api/        Rust crate(s): HTTP, domain modules, migrations, CLI
apps/web/        TanStack Start app: routes/public, routes/admin, components/ui (shadcn)
infra/           Ansible roles, Quadlet templates, example inventory, install guide
docs/adr/        Architecture decision records
docs/superpowers/specs/  Design documents
CONTEXT.md       Glossary
justfile         Single entry point: dev, test, check, gen, deploy
```

## 6. Decisions

Every decision has an ID. The implementation plan must reference each ID from at least one task, and each task must reference at least one ID (D-42). "Check" says how the decision is verified; "test" means an automated test, "CI" a CI job or lint rule, "review" a human check listed in the PR template.

### 6.1 Product and scope

**D-01 One Installation per Village, one VPS per Installation.** The maintainer operates Installations as a paid service; villages pay their own hosting; anyone may self-host. *Check:* ADR 0001; install guide (D-54).

**D-02 No village data in code.** Village identity is entered in a first-run setup wizard. The Administrator enters the Village Code; name, district, regency, province and time zone are filled from a bundled dataset of Kemendagri region codes. Logo, primary colour, Hamlet naming (dusun, banjar, jorong, …), office hours, contact details and social links are settings. Demo content for Sidomukti lives only in a separate seed (D-56). *Check:* test that a fresh database with no seed starts the setup wizard; CI grep that fails on known demo strings (e.g. "Sidomukti", "Sukamaju") outside `seed/` and tests.

**D-03 R1 scope is the CMS.** In R1:
- Setup wizard; access, roles and audit.
- Village: profile (identity, overview text, facts, Vision and Mission with RPJMDes period), region tree, government (Positions, Tenures, Officials), Institutions, Indeks Desa Editions.
- Village map: office location and Landmarks (D-57).
- Website: Home Hero, Head's Welcome, News Posts, Announcements, Events, Pages, Albums, Important Contacts, Partners, Transparency Documents, APBDes Summaries, Quick Links, Categories.
- All public pages of the prototype except population data: home, profile, history and other Pages, Vision and Mission, region and map, government (including past village heads from Tenure history), APBDes, Indeks Desa, news list and detail, announcements, events, gallery and album, documents, contacts, search results, 404.
- Accessibility panel, SEO (sitemap, RSS, Open Graph), update and rollback, backups, monitoring, demo seed.

*Check:* plan coverage; e2e smoke test that visits every public route and every admin menu item.

**D-04 Hidden until later releases.** The home page "Data penduduk" section and the "Data Penduduk" menu item stay hidden until R2 provides real population figures; no manually typed population numbers exist anywhere. The "Layanan Desa" button is shown only when the village sets an external URL in settings, until R3/R4 provide an internal service. *Check:* e2e test that a fresh Installation shows neither.

**D-05 No WordPress import; new domain.** *Check:* none needed (non-goal), listed so nobody builds it.

**D-06 Mobile priorities.** The public site is designed mobile-first. The admin is desktop-first, except the News Post editor and photo upload, which must be comfortable on a phone. *Check:* Playwright runs public pages and the news editor flow at a 390×844 viewport; review.

**D-07 Performance under load.** With the web and API containers limited to 2 vCPU / 2 GB together with PostgreSQL, a k6 scenario of 300 concurrent visitors browsing public pages keeps p95 under 1 s and the error rate under 1 %. *Check:* CI job `load` (k6) against built images, failing the build.

**D-08 Page weight.** Lighthouse mobile performance ≥ 90 on home, news detail and gallery album. Initial JavaScript for public routes ≤ 120 KB gzip (provisional; may be tightened in R0 with a note in the PR). Public routes may not import admin modules. *Check:* Lighthouse CI; size-limit; import-boundary lint rule.

**D-09 Accessibility.** WCAG 2.2 AA. An accessibility panel on public pages (text size, high contrast, solid backgrounds, underlined links, wider spacing) is applied before first paint, as in the prototype. *Check:* axe in Playwright with zero violations on every public route and admin screen; e2e test of the panel.

### 6.2 Architecture

**D-10 The API owns data and rules.** Rust/Axum is the only component with database access; all business rules live there. *Check:* the web image has no database credentials; review.

**D-11 Rendering split.** Public routes are server-rendered by TanStack Start; admin routes use `ssr: false`. *Check:* e2e asserts public HTML contains content without JavaScript; review.

**D-12 Public HTML cache.** The SSR server keeps rendered public HTML in a bounded in-memory cache. The API sends a purge signal after any write that affects public pages; a short TTL caps staleness if a signal is lost. Responses for signed-in users are never cached. *Check:* integration test: publish → next public request shows the change; load test (D-07).

**D-13 OpenAPI is the contract.** The API generates its OpenAPI document with utoipa; the web client is generated with Orval (TanStack Query hooks); generated files are committed. *Check:* CI regenerates both and fails on any diff.

**D-14 Background jobs in PostgreSQL.** A jobs table consumed with `FOR UPDATE SKIP LOCKED`, used for image processing and cache purges. No Redis. *Check:* integration tests for retry and at-most-one worker per job.

**D-15 Media storage.** Uploaded files live on a local volume, under content-addressed names, served by Caddy with long-lived immutable cache headers. Storage sits behind an interface so object storage can be added later. *Check:* integration test; header check in e2e.

**D-16 Image pipeline.**
- On the client, photos are downscaled in the browser before upload (target ≤ 2560 px long side), so an 8 MB WhatsApp photo travels as about 1 MB.
- On the server, a background job auto-rotates by EXIF orientation, **strips all metadata (including GPS)**, produces width variants, and encodes lossy WebP through libwebp. The `image` crate alone only writes lossless WebP.
- Uploads accept JPEG, PNG and WebP up to 10 MB; documents accept PDF up to 20 MB.

*Check:* unit tests on fixtures (EXIF with GPS removed, orientation applied); e2e upload from the phone viewport. See ADR 0006.

**D-17 Rich text.** Tiptap JSON is stored; the server renders HTML using an allowlist of extensions (headings, bold, italic, lists, quote, link, table, image, YouTube). No raw HTML. YouTube embeds load only when clicked. *Check:* tests that unknown nodes and `javascript:` links are dropped; Lighthouse.

**D-18 Search.** PostgreSQL full-text search with the `simple` configuration plus `unaccent` and `pg_trgm` (PostgreSQL has no Indonesian stemmer), across News Posts, Pages, Announcements, Events and Transparency Documents. *Check:* integration tests with Indonesian queries, including typos.

**D-19 SEO.** `sitemap.xml`, RSS for News Posts, canonical URLs, Open Graph and Twitter cards with the cover image, structured data for articles and events, and a real 404. *Check:* e2e and unit tests.

**D-20 Self-hosted fonts.** Geist is bundled (e.g. via Fontsource); no requests to Google Fonts or other third parties from public pages. *Check:* e2e asserts no third-party requests on public pages, except YouTube after a click.

### 6.3 Domain rules

**D-21 Records in use are deactivated, never deleted.** This covers Region Units, Users, Tenures, Positions and Officials. *Check:* tests per aggregate.

**D-22 One active Tenure per Position.** A Position has at most one active Tenure at a time, whatever its kind (definitive, Pj., Plt.). *Check:* test plus database constraint.

**D-23 Publication workflow.**
- News Posts, Announcements, Events, Pages, Albums, Important Contacts and Transparency Documents move Draft → Submitted → Published, and can be unpublished back to Draft.
- Users with `content.write` but without `content.publish` can create items, edit their own items while Draft or Submitted, and submit them. They cannot edit Published items or other people's items.
- Users with `content.publish` edit any item directly, and an edit to a Published item is live at once. There are no revisions.
- Single-record and list content without a status (Home Hero, Head's Welcome, Partners, Quick Links, Categories, Contact Categories) is live when saved and needs `content.publish`.
- The dashboard counts Submitted items as "Konten menunggu terbit".

*Check:* tests on the state machine and permissions.

**D-24 Head's Welcome follows the current village head.** It is signed by the active Tenure of the village head Position. When that Tenure has ended, the admin shows a warning and the public site still shows the text with the signer's real name and period. *Check:* test.

**D-25 Stable URLs.** A slug is fixed when the item is first published. Changing the title later does not change the URL; if an editor changes a slug explicitly, the old one redirects (301). *Check:* tests.

**D-26 Time-bound content.** An Announcement is shown only within its validity period, in the village's time zone. A cancelled Event stays listed with a "Dibatalkan" label. *Check:* tests with a fixed clock.

**D-27 APBDes Summary.** One per fiscal year, with one or more versions:
- **Versions.** The initial APBDes and each amended APBDes (Perubahan) is its own version with its adoption date. Earlier versions are kept, never overwritten. The public site shows the latest version and names it ("Versi Perubahan, ditetapkan …").
- **Budget per version:** income per source (PAD, Dana Desa, bagi hasil pajak dan retribusi, ADD, bantuan keuangan provinsi, bantuan keuangan kabupaten/kota, pendapatan lain-lain), spending per each of the five bidang, financing receipts and financing expenditures.
- **Realisation**, optional, per semester and cumulative: Semester I up to 30 June, Semester II up to 31 December, following the Laporan Realisasi. It is recorded per income source, per bidang, and for both financing lines.
- **Overruns.** Realisation may exceed the budget (for example income above target); the admin shows a warning, not an error.
- **Computation.** Totals, net financing and percentages are computed, never typed.
- **Amounts.** Whole rupiah stored as integers (never floats); none is negative.
- **Permission.** Editing requires the separate Permission `budget.manage`, so web contributors cannot change budget figures.

*Check:* unit tests (TDD).

**D-28 Optional categories.** News Posts and Events have one Category, defaulting to a built-in "Umum". *Check:* test.

**D-29 Indeks Desa Editions.** Each Edition stores its scale (IDM 0–1 for years before 2024, Indeks Desa 0–100 after), its status and its dimension scores. *Check:* tests.

### 6.4 Access and security

**D-30 Authentication.**
- Password hashed with argon2id, or passkey through webauthn-rs.
- Sessions stored server-side in PostgreSQL, in an HttpOnly, Secure, SameSite cookie.
- CSRF protection on state-changing requests.

*Check:* integration tests.

**D-31 Sudo mode.** Re-authentication within the last 10 minutes is required to manage Users and Roles, issue Access Links, and delete published content. *Check:* tests.

**D-32 No email; Access Links.** New Users and password resets go through single-use, short-lived Access Links that an Administrator creates and hands over (for example via WhatsApp). The first Administrator's link is created with a server CLI during installation. A registered passkey is a second way back in. *Check:* tests; install guide. See ADR 0007.

**D-33 Permissions and Roles.** Pranata defines a fixed Permission catalogue; each village defines its own Roles from it. The R1 catalogue is the prototype's R1 entries (`village.manage`, `content.write`, `content.publish`, `audit.read`, `user.manage`, `role.manage`) plus `budget.manage` (D-27). The built-in Administrator Role holds every Permission and cannot be edited. There is always at least one active Administrator. Each User has exactly one Role. Usernames are never reused. *Check:* tests.

**D-34 Login throttling.** Failed logins are rate-limited per account and per IP. *Check:* test.

**D-35 Audit Log.** Every change writes an Audit Entry (who, what, which object, when, before/after of changed fields). The application's database role has only `INSERT`/`SELECT` on the audit table. *Check:* test that `UPDATE`/`DELETE` on it fails for the app role.

**D-36 Security headers.** A Content-Security-Policy with a per-response nonce, HSTS, `X-Content-Type-Options` and a strict `Referrer-Policy`. *Check:* e2e header assertions.

**D-37 Observability.** Every request gets a request ID, logged with `tracing` as JSON and returned in a response header. *Check:* test.

### 6.5 Quality

**D-38 UI components only through `components/ui`.** shadcn components (Base UI variant) are installed in `apps/web/src/components/ui`. Feature code may not use raw `<button>`, `<input>`, `<select>`, `<textarea>` or `<dialog>`; a Biome GritQL rule enforces this. The visual language follows the prototype (admin: shadcn neutral; public: the prototype's green theme, driven by the village's primary colour). *Check:* CI lint; ADR 0008. This is the direct answer to archive issue #77.

**D-39 TDD for domain rules.** D-21 to D-29, D-31 and D-33 are written test-first. Rust integration tests run against a real PostgreSQL (`sqlx::test`). SQL is written with SQLx's compile-time checked macros (`query!`, `query_as!`, `query_scalar!`), with the offline data in `.sqlx/` committed and verified by `cargo sqlx prepare --check`; runtime-built queries are allowed only where SQL must be assembled dynamically, with a comment saying why. *Check:* CI `test` job; review.

**D-40 End-to-end tests.** Playwright runs against the built container images, with axe, Lighthouse CI and size-limit. *Check:* CI jobs `e2e`, `perf`.

**D-41 Static checks.**
- TypeScript, JSON and YAML: Biome.
- Rust: rustfmt, and clippy with the pedantic group as errors.
- `unsafe_code = "forbid"` in our crates.
- cargo-deny for advisories and AGPL-compatible licences.

*Check:* CI `lint`.

**D-42 Decision traceability.** A script checks that every `D-xx` in this document appears in the implementation plan, and that every plan task cites at least one `D-xx`. It runs in CI. The PR template asks which decisions a PR implements. *Check:* CI job `trace`; ADR 0004.

### 6.6 Workflow

**D-43 Monorepo with one entry point.** `apps/api`, `apps/web` and `infra` live in one repository. `just` is the single entry point (`dev`, `test`, `check`, `gen`, `deploy`, `seed`). *Check:* CI calls only `just` recipes.

**D-44 Trunk-based workflow.**
- Protected `main`, changed only through PRs with green CI; never pushed to directly.
- Atomic Conventional Commits, enforced by commitlint in lefthook and in CI.
- DCO `Signed-off-by` on every commit (ADR 0002). Licence AGPLv3.

*Check:* CI `commits` job; branch protection.

**D-45 CI on GitHub Actions.**
- Every action is pinned to a commit SHA.
- Jobs: `lint`, `typecheck`, `test` (unit + integration), `contract` (D-13), `trace` (D-42), `infra-lint` (ansible-lint, Quadlet validation), `build` (images), `e2e`, `perf`, `load`, `commits`.

*Check:* the workflow file itself; required checks on `main`.

**D-46 Releases.** release-please with one product version for API and web, a CHANGELOG and semver. Images are published to GHCR tagged with the version. Renovate updates dependencies. *Check:* release workflow.

**D-47 Domain documentation and issues.** `CONTEXT.md` holds the glossary; `docs/adr/` holds decisions. Issues live in GitHub Issues with triage labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). *Check:* review.

### 6.7 Infrastructure and operations

**D-48 Runtime.** Debian 13, rootless Podman with Quadlet/systemd, Caddy for TLS and as reverse proxy, distroless non-root images. *Check:* infra lint; image scan in CI.

**D-49 Provisioning.**
- One Ansible run sets up an Installation: nftables firewall, SSH by key only, unattended-upgrades, services, backups and monitoring.
- Real inventories (domains, IPs, secrets) live outside the repository; the repository holds an example inventory.

*Check:* CI runs the role against a disposable Debian 13 VM or container.

**D-50 Update and rollback.**
- One command moves one Installation, or all of them, to a given version.
- Migrations run on API start and follow expand/contract, so the previous version still runs on the new schema, and rolling back means starting the previous image.

*Check:* CI test: migrate to N, start N-1, smoke test.

**D-51 Backups.** restic, encrypted, to object storage outside the VPS. A nightly `pg_dump` and the media volume are backed up, with retention of 7 daily, 4 weekly and 12 monthly copies. A monthly automated restore check runs into a scratch database. *Check:* CI test of backup + restore against a local S3-compatible server; restore runbook in the install guide.

**D-52 Monitoring.** Each Installation reports: site up, last backup succeeded, disk below 80 %. The tool is the builder's call (section 9). *Check:* install guide; e2e of the health endpoint.

**D-53 Encryption at rest deferred to R2.** No LUKS/Clevis/Tang in R1. Backups are encrypted from R1. R2 decides between disk encryption and column-level encryption of sensitive fields before population data arrives. *Check:* ADR 0005.

**D-54 Installable by a stranger.** `infra/README.md` takes someone from a fresh VPS and a domain to a running Installation with its first Administrator, without help. *Check:* before R1 is released, someone other than the author follows the guide on a fresh VPS, and the result is recorded in the release PR.

### 6.8 Delivery

**D-55 Walking skeleton first (R0).** Before any R1 feature work, every stack piece of R1 works end to end on one thin slice:
- the Village profile is edited in the admin with Tiptap and a photo upload, and appears on the public home page through the cache;
- the slice is deployed by Ansible to a staging VPS, with backups running;
- every CI job of D-45 is active, including `trace`, `load`, axe and Lighthouse;
- shadcn components and the D-38 lint rule are in place.

*Check:* R0 exit checklist in the plan, ticked in the PR that closes R0.

**D-56 Demo seed.** `just seed demo` fills an empty Installation with synthetic content for Desa Sidomukti (real village identity from the region dataset; invented people, news and figures). The seed is never run automatically in production. *Check:* test that the seed runs on an empty database and refuses a non-empty one.

**D-57 Village map on OpenStreetMap.**
- The region page and the home page region section show an interactive Leaflet map with the village office pin, the Village's Landmarks, and the village boundary when a boundary is available (GeoJSON uploaded by an Administrator or taken from an open boundary dataset).
- The page also lists facts: area, elevation, distance to the district office, office coordinates and borders.
- The map loads only when the visitor taps it or scrolls to it, behind a static placeholder, so it adds nothing to the initial JavaScript budget (D-08) and makes no third-party request before interaction (D-20).
- The tile source is a Village Setting (OpenStreetMap by default; a satellite source may be configured). Attribution is always shown.
- A "Petunjuk arah" button opens directions to the office in the visitor's own map app (Google Maps URL with coordinates). The contact page's "Buka di peta" uses the same link.
- The admin has a "Peta" screen: click the map to set the office location, and add, edit, deactivate or order Landmarks (name, category, coordinates).

*Check:* e2e (map loads on interaction only, directions link has the right coordinates); Lighthouse unaffected.

**D-58 Home page sections.** In order:
1. Hero with search and Quick Links.
2. Current Announcement bar.
3. Quick-access tiles.
4. Latest news.
5. Upcoming events.
6. Gallery.
7. APBDes summary.
8. Region.
9. Head's Welcome and officials.
10. Latest documents.
11. Important Contacts.
12. Partners.

A section with no content is hidden, never shown empty. *Check:* e2e on a fresh Installation and with the demo seed.

**D-59 Infrastructure prerequisites are R0 tasks.**
- The repository is public on GitHub from day one, which also gives 4-vCPU hosted runners for D-07.
- The archived repository currently named `dodoarmando/pranata` is renamed first, so the new repository can take that name.
- A staging VPS, a domain and an S3-compatible object storage account do not exist yet; procuring them (by the maintainer, guided by the plan) is part of R0.

*Check:* R0 exit checklist.

**D-60 Deploys run from the operator's machine.** `just deploy` runs Ansible from the maintainer's (or any operator's) computer. CI builds and publishes images but holds no SSH keys to village servers. *Check:* the workflow files contain no deploy job or SSH secret.

## 7. Releases

| Release | Content | Exit |
|---|---|---|
| **R0 Walking skeleton** (internal) | D-55 | R0 checklist ticked; `trace` green |
| **R1 CMS** | D-01 … D-60 | Sidomukti is live on Pranata and has left WordPress; D-54 verified |
| **R2 Population** | Resident, Family, Population Event, Baseline import (Prodeskel / OpenSID formats), frozen Monthly Recap, registers B.1–B.4, Temporary Stay, Retention Schedule and Destruction, encryption-at-rest decision, public population section | — |
| **R3 Letters** | Letter Types and Layouts, Service Letters (Typst + QR verification), Recorded Letters, yearly Letter Number sequence, Outgoing Letter Agenda, public verification page | — |
| **R4 Resident services** | Online Letter Requests, Complaints, APBDes import from Siskeudes, optional email | — |

R2 and R3 are separate because letters depend on stable Resident data.

**Pace.** There is no hard deadline. The maintainer builds Pranata alone, with AI agents, as a portfolio and hobby project. The target is for Sidomukti to have its own website on Pranata during 2027. So R1 follows this document in full rather than a cut-down go-live list, and plan tasks are sized for agent execution: one PR each, with explicit acceptance checks. After the product is stable, features that support the maintainer's own village work will be designed as a later release. The archive's ADRs on frozen Monthly Recaps and approved Destruction are inputs to R2's design, not decisions of R1.

## 8. Stack (versions checked 2026-11-24)

Renovate keeps these current. The plan re-checks each version against its registry when a package is first added.

| Area | Choice | Version |
|---|---|---|
| Language | Rust, edition 2024 | 1.100.0 |
| HTTP | axum / tokio / tower-http | 0.8.9 / 1.54.1 / 0.7.1 |
| Database | PostgreSQL / SQLx | 19.0 / 0.9.1 |
| API contract | utoipa / utoipa-axum | 6.0.0 / 0.3.0 |
| Auth | webauthn-rs / argon2 | 0.6.2 / 0.6.0 |
| Throttling | tower_governor | 0.8.0 |
| Logging | tracing / tracing-subscriber | 0.1.44 / 0.3.23 |
| Images | image / fast_image_resize / libwebp binding (e.g. `webp`) / kamadak-exif | 0.25.10 / 6.2.0 / 0.3.1 / 0.6.2 |
| Runtime | Bun | 1.4.5 |
| UI | React / TypeScript | 19.3.1 / 7.0.2 |
| Framework | TanStack Start / Router / Query / Table | 1.169.7 / 1.170.46 / 5.108.4 / 9.3.1 |
| Build / unit test | Rsbuild / Rstest (fallback: Vite 8.3 / Vitest 5.1) | 2.3.3 / 0.12.6 |
| Styling | Tailwind CSS / shadcn (Base UI) / Base UI / lucide-react | 4.3.5 / 4.23.0 / 1.9.0 / 1.78.0 |
| Rich text | Tiptap | 3.35.0 |
| Map | Leaflet (loaded on demand) | 1.9.4 |
| API client | Orval | 8.49.0 |
| Lint | Biome | 2.6.8 |
| E2E / a11y / perf | Playwright / axe-core / Lighthouse CI / size-limit / k6 | 1.65.1 / 4.14.0 / 0.16.1 / 14.3.0 / 2.4.0 |
| Supply chain | cargo-deny | 0.22.0 |
| Hooks / commits / release | lefthook / commitlint / release-please | 2.2.2 / 21.2.4 / 17.11.2 |
| OS / containers / proxy | Debian / Podman / Caddy | 13.8 / 5.4 (Debian package) / 2.11.9 |
| Backups | restic | 0.20.0 |

Deferred to R2/R3 and **not** installed in R0/R1: Typst 0.15.1, calamine 0.38.0, rust_xlsxwriter 0.104.0.

## 9. Left to the builder

These are settled during planning or R0, and recorded in the PR (or an ADR if the choice is hard to reverse):

- The region dataset source. It must follow the latest Kepmendagri on region codes (currently No. 300.2.2-2430/2025) and have an AGPL-compatible licence. Candidates: `cahyadsn/wilayah`, `yonatanyl/KODE-WILAYAH-KEPMENDAGRI-2025` (CC BY 4.0, needs attribution). Updating the dataset is a data migration.
- The cache implementation details behind D-12 (keying, size bound, TTL value, purge transport).
- The monitoring tool for D-52 and where its dashboard runs.
- The object storage provider for D-51 (prefer storage located in Indonesia).
- Exact image variant widths and WebP quality.
- Where the admin menu places Village Settings, Quick Links, Categories and Contact Categories (they are not in the prototype), and which fixed parent menus Pages may sit under.
- The precise k6 scenario mix (D-07), as long as it covers home, news list and detail, gallery album and search.
- Page copy and empty states, following the prototype's tone.
- Splitting `apps/api` into several crates or keeping one crate with modules.

## 10. Calls made during writing

The maintainer did not discuss these points directly. They are the author's proposals and can be corrected:

- D-08 budget numbers, D-31's 10-minute window, D-51 retention counts, D-16 size limits.
- D-04: the "Layanan Desa" button shown only when an external URL is set.
- D-57: OpenStreetMap via Leaflet chosen by the maintainer; the configurable tile source and the boundary sources are the author's proposal.
- D-58: the order of home page sections, taken from the prototype.
- D-23: no revisions; direct, live edits by publishers; contributors limited to their own unpublished items; status-less content needs `content.publish`.
- D-27/D-33: the new `budget.manage` Permission.
- D-60: deploys from the operator's machine, not from CI.
- Login uses a username (per the archive's glossary); the prototype's email field becomes "Nama pengguna".
