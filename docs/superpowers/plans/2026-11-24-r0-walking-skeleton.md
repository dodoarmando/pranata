# R0 Walking Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every R1 stack piece working end to end on one thin slice. An Administrator signs in, edits the Village profile overview with Tiptap, uploads a hero photo from a phone, and sees both on the public home page, served through the purged HTML cache. The slice is deployed by Ansible to a staging VPS with encrypted backups, and every CI gate is active.

**Architecture:** One Rust crate `pranata` (`apps/api`) owns PostgreSQL and every rule. It exposes `/api/*`, runs migrations and a Postgres-backed job worker inside `serve`, and processes images. A TanStack Start app (`apps/web`) on Bun server-renders public routes behind an in-memory HTML cache and runs admin routes client-only. Caddy routes `/api/*` to the API, `/media/*` to the media volume, and everything else to the web server. Ansible installs one Installation per VPS as rootless Podman Quadlet units.

**Tech Stack:** See spec section 8 (Rust 1.100, axum 0.8, SQLx 0.9, PostgreSQL 19, utoipa 6, Bun 1.4, TanStack Start 1.169, Rsbuild 2.3, Tailwind 4.3, shadcn 4.23 (Base UI), Tiptap 3.35, Orval 8.49, Biome 2.6, Playwright 1.65, k6 2.4, restic 0.20, Debian 13, Podman 5.4, Caddy 2.11). Before adding any package, re-check its latest stable version on its registry and use that.

**Spec:** `docs/superpowers/specs/2026-11-24-pranata-r1-cms-design.md` (decisions D-01…D-60). Also read `CONTEXT.md` and `docs/adr/`.

## Global Constraints

- Every task lists the decisions it implements on a `**Decisions:**` line. The trace checker (Task 2) reads these lines.
- UI copy is Indonesian. Code, identifiers, commits, docs and API error `code`/`title` are English. The web app maps `code` to Indonesian text.
- Commits are atomic Conventional Commits with a DCO sign-off: `git commit -s`. Allowed scopes: `api`, `web`, `infra`, `ci`, `tools`, `deps`, `adr`, `spec`, `plan`, `release`. Every commit message ends with the `Co-Authored-By` trailer the session supplies.
- Never commit `.prototype/`. It is a local-only visual reference, excluded through `.git/info/exclude`.
- No village-specific data in code (D-02). The words `Sidomukti` and `Sukamaju` may appear only under `apps/api/seed/`, test fixtures and `docs/`.
- `unsafe_code = "forbid"`. Clippy pedantic warnings are errors.
- SQL uses SQLx compile-time checked macros (`query!`, `query_as!`, `query_scalar!`) with committed `.sqlx/` offline data; runtime queries only where SQL must be built dynamically, with a comment (D-39).
- Money is integer rupiah. No floats for money anywhere (D-27; first used in R1).
- Public pages make no third-party requests (D-20).
- Feature code never renders raw `<button>`, `<input>`, `<select>`, `<textarea>` or `<dialog>` (D-38).
- Performance targets: p95 < 1 s and errors < 1 % at 300 concurrent visitors within 2 vCPU / 2 GB (D-07). Lighthouse mobile performance ≥ 90; public initial JS ≤ 120 KB gzip (D-08).
- Upload limits: images JPEG/PNG/WebP ≤ 10 MB; PDF ≤ 20 MB (D-16).
- Main is protected. Work happens on branches through PRs. The plan's tasks map 1:1 to PRs unless noted.

## Review Focus

1. **Unsupported photo format** (an iPhone HEIC, or a renamed `.txt`) → the API answers `415` with code `unsupported_media_type`, and the admin shows "Format foto tidak didukung. Gunakan JPEG, PNG, atau WebP." It never fails with a 500. Test in Task 7.
2. **The same photo uploaded twice** (a retry after a dropped connection) → the same Media Asset is returned (deduplicated by SHA-256), with no duplicate files or jobs. Test in Task 7.
3. **Purge signal lost because the web server is down when content is saved** → the purge job retries with backoff, and the cache TTL (300 s) caps staleness. Test in Tasks 9 and 11.
4. **An Access Link opened twice, or after expiry** → the second use answers `410` with code `access_link_used`, or `access_link_expired`. The page tells the user to ask an Administrator for a new link. Test in Task 6.
5. **A decompression bomb** (a PNG declaring 30000×30000 pixels) → rejected before decoding, with code `image_too_large`. The limit is 50 megapixels. Test in Task 7.

---

## File structure (end of R0)

```
apps/api/
  Cargo.toml                 crate `pranata` (lib + bin)
  Containerfile
  migrations/                SQLx migrations (owner role applies them)
  seed/                      demo seed data (R1, Plan R1-E)
  data/region-codes.csv.gz   bundled Kepmendagri dataset
  src/
    main.rs                  CLI entry (clap)
    lib.rs                   app(), AppState
    config.rs                Config::from_env()
    error.rs                 ApiError → problem+json
    http/{mod,request_id,security_headers}.rs
    db.rs                    pools, migrate()
    openapi.rs               ApiDoc
    audit.rs                 record(), NewAuditEntry
    region_codes.rs          VillageCode, lookup()
    village/{mod,profile}.rs
    auth/{mod,password,session,access_link,csrf}.rs
    jobs.rs                  enqueue(), worker
    media/{mod,storage,pipeline,upload}.rs
    rich_text.rs             RichTextDoc, validate(), to_html()
    cache_purge.rs           Purger
  tests/                     integration tests (sqlx::test)
apps/web/
  package.json, rsbuild.config.ts, orval.config.ts, components.json, Containerfile
  src/
    router.tsx, routeTree.gen.ts
    server/{entry.ts,html-cache.ts,csp.ts}
    api/                     Orval output (generated, committed)
    components/ui/           shadcn (only place for raw controls)
    components/rich-text-editor.tsx
    lib/{downscale-image.ts,errors.ts}
    routes/__root.tsx, index.tsx, masuk.tsx, akses.tsx
    routes/admin/route.tsx, admin/index.tsx, admin/desa/profil.tsx
    admin/                   admin-only modules (public routes may not import)
    styles.css
  e2e/                       Playwright specs + fixtures
  biome-plugins/no-raw-controls.grit
load/public.js               k6 scenario
tools/trace/{trace.ts,trace.test.ts}
infra/
  provision.yml, deploy.yml, inventory.example.yml, README.md
  roles/pranata/{tasks,templates,handlers,defaults}/
  test/Containerfile
.github/{workflows/ci.yml,workflows/release.yml,pull_request_template.md}
justfile, lefthook.yml, commitlint.config.ts, biome.json, Cargo.toml, rust-toolchain.toml,
deny.toml, renovate.json, release-please-config.json, .release-please-manifest.json, LICENSE, README.md
```

---

### Task 0: GitHub prerequisites

**Decisions:** D-59, D-44

These are outward-facing actions. The executor runs each command only after the maintainer confirms it in the session.

- [ ] **Step 1: Rename the archived repository.** Run `gh repo rename pranata-archive -R dodoarmando/pranata --yes`, then `gh repo archive dodoarmando/pranata-archive --yes`. Expected: `gh repo view dodoarmando/pranata-archive --json isArchived` prints `true`.
- [ ] **Step 2: Create the public repository and push.** `gh repo create dodoarmando/pranata --public --source . --remote origin --description "Village website and administration for Indonesian villages"`, then `git push -u origin main` and `git push -u origin docs/r1-design`. Open a PR for `docs/r1-design` and merge it once Task 1 lands (Task 1's CI is the first gate).
- [ ] **Step 3: Repository settings.** Squash merging off, rebase merging on (commits stay atomic), auto-delete branches on. Enable GitHub Issues and create the five triage labels (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`) with `gh label create`. Branch protection comes in Task 15, Step 4, once the CI jobs exist.

### Task 1: Repository tooling

**Decisions:** D-43, D-44, D-41, D-47, D-02

**Files:**
- Create: `Cargo.toml` (workspace, `members = ["apps/api"]`, `[workspace.lints.rust] unsafe_code = "forbid"`, `[workspace.lints.clippy] pedantic = { level = "deny", priority = -1 }`), `rust-toolchain.toml` (channel `1.100.0`, components rustfmt + clippy), `package.json` (Bun workspaces `["apps/web", "tools/*"]`), `biome.json`, `lefthook.yml`, `commitlint.config.ts`, `justfile`, `LICENSE` (AGPL-3.0 full text), `README.md`, `.gitignore`, `.github/pull_request_template.md`, `tools/no-village-data.sh`

- [ ] **Step 1: commitlint config.** Extend `@commitlint/config-conventional`. Set `scope-enum` to the Global Constraints list. Add a custom rule `signed-off-by` that requires a `Signed-off-by:` trailer. Verify with `echo "feat: x" | bunx commitlint` → FAIL (no sign-off), and `printf "feat(api): x\n\nSigned-off-by: A <a@b.c>" | bunx commitlint` → PASS.
- [ ] **Step 2: lefthook.** `commit-msg` runs commitlint. `pre-commit` runs `bunx biome check --staged` and `cargo fmt --check` when `*.rs` files are staged.
- [ ] **Step 3: PR template.** It holds a required `## Decisions` section ("Which D-xx does this PR implement or change?"), a `## Checks` list, and an R0/R1 exit-checklist pointer.
- [ ] **Step 4: `tools/no-village-data.sh`.** The script greps tracked files for `Sidomukti|Sukamaju` (case-insensitive). Allowed paths: `apps/api/seed/`, `**/fixtures/`, `docs/`, `CONTEXT.md`. Exits 1 on any other hit and lists the hits. Verify: create a temp tracked file containing "Sukamaju", run → exit 1; remove it → exit 0.
- [ ] **Step 5: justfile skeleton.** `default` lists recipes. Placeholder recipes are filled by later tasks: `install`, `dev`, `db-up`, `db-migrate`, `gen`, `test`, `check`, `check-commits`, `check-api`, `check-web`, `check-contract`, `check-trace`, `check-infra`, `check-village-data` (runs Step 4), `images`, `e2e`, `perf`, `load`, `seed`, `deploy`. Verify with `just --list`.
- [ ] **Step 6: Commit** in three commits: `build: add workspace, toolchain and lint configuration`, `build: enforce conventional commits with DCO sign-off`, `chore: add licence, readme and PR template`.

### Task 2: Decision trace checker

**Decisions:** D-42

**Files:** Create `tools/trace/package.json`, `tools/trace/trace.ts`, `tools/trace/trace.test.ts`

**Interfaces:**
- Produces: CLI `bun tools/trace/trace.ts [--strict]`.
  - Reads `docs/superpowers/specs/*.md` for decision headings matching `^\*\*(D-\d{2,3}) `.
  - Reads `docs/superpowers/plans/*.md` for task `**Decisions:**` lines, roadmap table rows (`| R1-A … | D-xx, D-yy |`) and the `**Not built:**` line.
  - Exit 0 or 1, with a report on stdout.
- `export function check(specs: string[], plans: string[], strict: boolean): { ok: boolean; missing: string[]; tasksWithoutDecisions: string[]; unknown: string[] }`

- [ ] **Step 1: Write failing tests** (`bun test tools/trace`):
  - `reports a decision cited nowhere` → `missing == ["D-02"]` when the spec has D-01 and D-02 and the plan cites only D-01.
  - `accepts roadmap citations in default mode`.
  - `rejects roadmap-only citations in strict mode`.
  - `accepts Not built citations in both modes`.
  - `reports a task heading without a Decisions line` → `tasksWithoutDecisions == ["Task 3: Foo"]`.
  - `reports unknown ids` → a plan citing D-99 that no spec defines.
- [ ] **Step 2: Run** `bun test tools/trace` → FAIL (module missing).
- [ ] **Step 3: Implement `check()`** and the CLI wrapper. A task heading is `^### Task \d+:`. The Decisions line must appear before the next `###`. Ranges like `D-01…D-60` are not allowed; each ID is listed individually.
- [ ] **Step 4: Run** `bun test tools/trace` → PASS. Then run `bun tools/trace/trace.ts` against this repo → exit 0.
- [ ] **Step 5: Wire** `just check-trace` (default mode) and `just check-trace-strict`.
- [ ] **Step 6: Commit** `feat(tools): check that every design decision is traced to a plan task`.

### Task 3: API skeleton, observability and contract

**Decisions:** D-10, D-13, D-37, D-36, D-39, D-50

**Files:** Create `apps/api/Cargo.toml`, `src/{main,lib,config,error,db,openapi}.rs`, `src/http/{mod,request_id,security_headers}.rs`, `migrations/0001_roles.sql`, `tests/health.rs`, `infra/dev/postgres-init.sql`. Modify `justfile`.

**Interfaces:**
- Produces:
  - `pub struct Config { database_url: String, migration_database_url: String, public_origin: Url, web_internal_url: Url, purge_token: SecretString, media_dir: PathBuf, listen: SocketAddr }` with `Config::from_env() -> Result<Config, ConfigError>`. Environment variables: `PRANATA_DATABASE_URL`, `PRANATA_MIGRATION_DATABASE_URL`, `PRANATA_PUBLIC_ORIGIN`, `PRANATA_WEB_INTERNAL_URL`, `PRANATA_PURGE_TOKEN`, `PRANATA_MEDIA_DIR`, `PRANATA_LISTEN` (default `0.0.0.0:8080`).
  - `pub struct AppState { pub db: PgPool, pub config: Arc<Config> }`. Task 7 adds `pub storage: Arc<dyn media::Storage>`; Task 9 adds `pub purger: cache_purge::Purger`.
  - `pub fn app(state: AppState) -> axum::Router`, mounted under `/api`.
  - `pub enum ApiError { NotFound(&'static str), Unauthorized, Forbidden, Conflict(&'static str), Gone(&'static str), Unsupported(&'static str), Validation(Vec<FieldError>), Internal(anyhow::Error) }`. It renders RFC 9457 `application/problem+json` as `{type, title, status, code, errors?}`; `code` is the `&'static str`, in snake_case.
  - `pub async fn db::migrate(owner_url: &str) -> Result<()>`, run at `serve` start before binding.
  - CLI (clap): `pranata serve | migrate | openapi`.
- Database roles: `infra/dev/postgres-init.sql` creates the login roles `pranata_owner` (owns the schema) and `pranata_app`. Migration `0001_roles.sql` sets `ALTER DEFAULT PRIVILEGES … GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO pranata_app`.

- [ ] **Step 1: Write failing integration tests** in `tests/health.rs`, using `#[sqlx::test]` and `tower::ServiceExt::oneshot`:
  - `healthz_reports_version` → `GET /api/healthz` returns 200 with `{"status":"ok","version": env!("CARGO_PKG_VERSION")}`.
  - `every_response_has_a_request_id` → the `x-request-id` header is a UUID, and an incoming `x-request-id` is echoed back.
  - `unknown_route_is_problem_json` → 404 with content type `application/problem+json` and `code == "not_found"`.
  - `security_headers_present` → `strict-transport-security: max-age=31536000`, `x-content-type-options: nosniff`, `referrer-policy: strict-origin-when-cross-origin`, `x-frame-options: DENY`.
- [ ] **Step 2: Run** `just test-api` → FAIL.
- [ ] **Step 3: Implement.** Use tower-http `SetRequestIdLayer` + `PropagateRequestIdLayer` (`MakeRequestUuid`) and `TraceLayer`, with the request ID added to the span. Configure `tracing-subscriber` with the JSON formatter and an `EnvFilter` from `RUST_LOG`, default `info`. utoipa `ApiDoc` collects paths. `pranata openapi` prints JSON to stdout.
- [ ] **Step 4: justfile.** `db-up` starts `docker.io/library/postgres:19` as container `pranata-db` on `127.0.0.1:55432`, mounting `infra/dev/postgres-init.sql`; it is a no-op when `CI` is set. `db-migrate` runs `cargo run -q -- migrate`. `test-api` runs `cargo test --locked`. `gen` runs `cargo run -q -- openapi > apps/api/openapi.json`, then the web codegen (Task 10).
- [ ] **Step 5: Run** `just test-api` → PASS. `just check-api` (fmt, clippy pedantic, `cargo sqlx prepare --check`, tests) → PASS.
- [ ] **Step 6: Commit** `feat(api): serve health with request ids, json logs and problem responses`.

### Task 4: Audit Log with append-only grants

**Decisions:** D-35, D-39

**Files:** Create `apps/api/src/audit.rs`, `migrations/0002_audit.sql`, `tests/audit.rs`

**Interfaces:**
- Produces:
  - `pub struct NewAuditEntry<'a> { pub actor: Option<UserId>, pub action: AuditAction, pub object_type: &'a str, pub object_id: String, pub summary: String, pub changes: Vec<FieldChange> }`
  - `pub enum AuditAction { Create, Update, Publish, Unpublish, Submit, Deactivate, Delete, SignIn, SignOut }`, stored as snake_case text.
  - `pub struct FieldChange { pub field: String, pub before: serde_json::Value, pub after: serde_json::Value }`
  - `pub async fn record(conn: &mut PgConnection, entry: NewAuditEntry<'_>) -> Result<AuditEntryId>`, always called inside the same transaction as the change it records.
  - `pub fn diff<T: Serialize>(before: &T, after: &T) -> Vec<FieldChange>` returns only top-level fields that differ.
- Table `audit_entries(id uuid pk, at timestamptz default now(), actor uuid null, action text, object_type text, object_id text, summary text, changes jsonb)`. Migration: `REVOKE UPDATE, DELETE, TRUNCATE ON audit_entries FROM pranata_app;`.

- [ ] **Step 1: Failing tests:**
  - `record_inserts_entry`.
  - `diff_lists_only_changed_fields` → `{a:1,b:2}` vs `{a:1,b:3}` gives `[FieldChange{field:"b",before:2,after:3}]`.
  - `app_role_cannot_update_audit` → after `SET ROLE pranata_app`, `UPDATE audit_entries SET summary='x'` errors with SQLSTATE `42501`.
  - `app_role_cannot_delete_audit` → same check for `DELETE`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(api): record append-only audit entries`.

### Task 5: Village identity from the bundled region dataset

**Decisions:** D-02, D-39

**Files:** Create `apps/api/data/region-codes.csv.gz`, `apps/api/data/SOURCE.md`, `src/region_codes.rs`, `src/village/mod.rs`, `migrations/0003_village.sql`, `tests/village.rs`

**Interfaces:**
- Produces:
  - `pub struct VillageCode(String)`, with `VillageCode::parse(s: &str) -> Result<VillageCode, VillageCodeError>`. It accepts `14.05.03.2001` or `1405032001` and normalises to the dotted form. Input that is not 10 digits gives `VillageCodeError::Format`.
  - `pub struct RegionRecord { pub village: String, pub district: String, pub regency: String, pub province: String, pub time_zone: TimeZone }`
  - `pub fn lookup(code: &VillageCode) -> Option<RegionRecord>` reads the gzip CSV embedded with `include_bytes!` and parses it lazily once (`std::sync::LazyLock<HashMap>`).
  - `pub enum TimeZone { Wib, Wita, Wit }`, derived from the province code:
    - WITA: 51, 52, 53, 63, 64, 65, 71, 72, 73, 74, 75, 76.
    - WIT: 81, 82, and 91 to 97.
    - Everything else: WIB.
    - IANA names: `Asia/Jakarta`, `Asia/Makassar`, `Asia/Jayapura`.
  - CLI `pranata village init --code <code>`. It fails if a Village already exists, and inserts the single `village` row: `id smallint pk check (id = 1)`, code, names, time zone, `overview jsonb null`, `hero_asset_id uuid null`, `updated_at`.
  - `GET /api/public/village` → `VillageProfile { code, name, district, regency, province, time_zone }`, or 404 `village_not_initialized`. Task 9 adds `overview_html: Option<String>` and `hero: Option<MediaRef>`.
- Dataset: build the CSV from the latest Kepmendagri region codes (currently No. 300.2.2-2430/2025). Use a source whose licence is AGPL-compatible (the spec, section 9, names two candidates) and record the source, licence, attribution and retrieval date in `data/SOURCE.md`. Columns: `code,village,district,regency,province`.

- [ ] **Step 1: Failing tests:**
  - `parse_accepts_dotted_and_plain` (both forms give the same `VillageCode`).
  - `parse_rejects_nine_digits`.
  - `lookup_finds_a_riau_village` → a known code under `14.05.03` returns regency "Pelalawan", province "Riau", `TimeZone::Wib`. Take the expected code from the dataset; this lives in a test fixture, so it is allowed under D-02.
  - `lookup_maps_papua_to_wit`.
  - `public_village_404_before_init`.
  - `init_twice_fails`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** in two commits: `feat(api): bundle the Kepmendagri region codes` and `feat(api): initialise the Village from its code`.

### Task 6: Password sign-in, sessions, CSRF and Access Links

**Decisions:** D-30, D-32, D-35, D-39

**Files:** Create `apps/api/src/auth/{mod,password,session,access_link,csrf}.rs`, `migrations/0004_auth.sql`, `tests/auth.rs`

**Interfaces:**
- Tables:
  - `users(id uuid pk, username text unique, display_name text, password_hash text null, is_active bool default true, created_at)`.
  - `sessions(token_hash bytea pk, user_id, csrf_token text, created_at, last_seen_at, expires_at, remember bool)`.
  - `access_links(token_hash bytea pk, user_id, created_by uuid null, expires_at, used_at null)`.
- Produces:
  - `pub fn hash_password(p: &str) -> Result<String>` and `verify_password(p: &str, hash: &str) -> bool`, using argon2id with the crate's default params.
  - Password rule: 10 to 128 characters, no composition rules. Otherwise `Validation` with code `password_length`.
  - Session cookie `pranata_session`: 32 random bytes, base64url. The database stores its SHA-256. Flags: `HttpOnly; Secure; SameSite=Lax; Path=/`.
  - Without "remember": a session cookie (no Max-Age) with a server expiry of 12 h.
  - With "remember": `Max-Age=2592000`, sliding 30 days, renewed at most once per hour.
  - `pub struct CurrentUser { pub id: UserId, pub username: String, pub display_name: String }`, an axum extractor. A missing or expired session gives 401 `unauthenticated`.
  - CSRF: every non-GET `/api/admin/*` and `/api/auth/logout` request must send `x-csrf-token` equal to the session's `csrf_token`; otherwise 403 `csrf_mismatch`.
  - Endpoints:
    - `POST /api/auth/login {username, password, remember}` → 200 `{user, csrf_token}`. A wrong password or unknown user gives 401 `invalid_credentials`, the same response for both.
    - `POST /api/auth/logout` → 204.
    - `GET /api/auth/me` → `{user, csrf_token}`.
    - `POST /api/auth/access-links/{token}/password {password}` → 204. It sets the password and marks the link used. A used link gives 410 `access_link_used`; an expired one gives 410 `access_link_expired`; an unknown one gives 404 `access_link_not_found`.
  - CLI `pranata user create-admin <username> --name "<display name>"` creates the user and an Access Link valid for 24 h, then prints `{PRANATA_PUBLIC_ORIGIN}/akses#{token}`. The token travels in the URL fragment so no server or proxy log ever sees it; redemption is `POST /api/auth/access-links/redeem {token, password}`.
  - Sign-in and sign-out write Audit Entries (`SignIn`, `SignOut`).
  - Roles, passkeys, sudo mode and throttling come in Plan R1-A. In R0, every User has full access.

- [ ] **Step 1: Failing tests:**
  - `login_sets_session_cookie_and_returns_csrf`.
  - `wrong_password_and_unknown_user_look_identical` (same status and body).
  - `session_expires_after_12h_without_remember` (with a fixed clock via `SessionClock` injection).
  - `remember_extends_to_30_days`.
  - `admin_post_without_csrf_is_403`.
  - `access_link_sets_password_once` (second use → 410 `access_link_used`).
  - `expired_access_link_is_410`.
  - `short_password_rejected`.
  - `sign_in_writes_audit_entry`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(api): sign in with a password, server sessions and access links`.

### Task 7: Job queue and image pipeline

**Decisions:** D-14, D-15, D-16, D-39

**Files:** Create `apps/api/src/jobs.rs`, `src/media/{mod,storage,pipeline,upload}.rs`, `migrations/0005_jobs_media.sql`, `tests/media.rs`, `tests/fixtures/{gps.jpg,rotated-6.jpg,bomb.png,fake.heic,doc.pdf}`

**Interfaces:**
- Tables:
  - `jobs(id bigserial pk, kind text, payload jsonb, run_at timestamptz, attempts int default 0, last_error text null, done_at null)`.
  - `media_assets(id uuid pk, sha256 bytea unique, kind text check in ('image','document'), mime text, original_name text, bytes bigint, width int null, height int null, status text check in ('pending','ready','failed'), created_by uuid, created_at)`.
  - `media_variants(asset_id, width int, path text, bytes bigint, primary key(asset_id, width))`.
- Produces:
  - `pub enum Job { ProcessImage { asset_id: Uuid }, PurgePublicCache }`
  - `pub async fn jobs::enqueue(conn: &mut PgConnection, job: Job, run_at: DateTime<Utc>) -> Result<()>`
  - `pub async fn jobs::run_worker(state: AppState, shutdown: CancellationToken)` claims with `SELECT … FOR UPDATE SKIP LOCKED LIMIT 1`. Retry backoff is `min(2^attempts, 300)` seconds, up to 8 attempts, then `failed`. `serve` spawns one worker.
  - `#[async_trait] pub trait Storage: Send + Sync { async fn put(&self, rel: &str, bytes: Bytes) -> Result<()>; async fn get(&self, rel: &str) -> Result<Bytes>; }`, with `LocalStorage { root }`.
  - Path layout: originals at `originals/{hex[0..2]}/{hex}.{ext}`; variants at `public/{hex[0..2]}/{hex}-{w}.webp`. Caddy serves `/media/*` from the `public/` root.
  - `POST /api/admin/media` (multipart field `file`) → 201 `MediaAsset { id, kind, status, width, height, variants: Vec<MediaVariant { width, url }> }`. The same SHA-256 returns 200 with the existing asset.
  - `GET /api/admin/media/{id}` returns the same shape.
  - `pub struct MediaRef { id, alt: Option<String>, variants: Vec<MediaVariant> }` is used by public responses.
- Pipeline (`pipeline::process_image(original: &[u8]) -> Result<Processed>`):
  1. Sniff the type from magic bytes. Accept only JPEG, PNG and WebP; anything else gives 415 `unsupported_media_type`. PDF is accepted only with `kind=document` and is not image-processed.
  2. Read dimensions from the header without decoding. Above 50 MP, return 422 `image_too_large`.
  3. Decode, then apply the EXIF orientation.
  4. Re-encode the stored original without any metadata (JPEG stays JPEG).
  5. Variant widths are `[480, 960, 1600, 2560]`, never upscaled. Resize with fast_image_resize (Lanczos3) and encode lossy WebP at quality 78 via libwebp.
- Size limits are enforced on upload: images 10 MB give 413 `file_too_large`; PDFs 20 MB.

- [ ] **Step 1: Failing tests:**
  - `gps_exif_is_stripped` → no EXIF block in the stored original or in any variant (use kamadak-exif to read).
  - `orientation_6_is_applied` (output width/height swapped compared with the raw header).
  - `variants_never_upscale` (a 1000 px image gives variants 480 and 960 only, plus the original).
  - `heic_is_415`.
  - `bomb_is_rejected_before_decode` (code `image_too_large`, with peak memory not exercised: assert the error comes from the header check).
  - `same_file_twice_returns_same_asset` (one row, one job).
  - `job_retries_with_backoff` (a failing handler sets `run_at` ≈ now + 2^attempts s).
  - `two_workers_never_claim_the_same_job`.
  - `oversize_upload_is_413`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** in two commits: `feat(api): run background jobs from postgres` and `feat(api): store uploads and derive metadata-free webp variants`.

### Task 8: Rich text validation and rendering

**Decisions:** D-17, D-39

**Files:** Create `apps/api/src/rich_text.rs`, `tests/rich_text.rs`

**Interfaces:**
- Produces:
  - `pub struct RichTextDoc(serde_json::Value)`, ProseMirror JSON as Tiptap emits it.
  - `pub fn validate(doc: &serde_json::Value) -> Result<RichTextDoc, RichTextError>`
  - `pub fn to_html(doc: &RichTextDoc, media: &dyn Fn(Uuid) -> Option<MediaRef>) -> String`
- Allowlist:
  - Nodes: `doc`, `paragraph`, `heading` (levels 2 to 4), `bulletList`, `orderedList`, `listItem`, `blockquote`, `hardBreak`, `table`, `tableRow`, `tableHeader`, `tableCell`, `image` (attrs: `assetId`, `alt`), `youtube` (attr: `videoId` matching `^[A-Za-z0-9_-]{11}$`), `text`.
  - Marks: `bold`, `italic`, `link` (only `http`, `https`, `mailto` and `tel`; rendered with `rel="noopener nofollow"`).
  - Unknown nodes and marks are dropped and their text children kept. Disallowed link schemes drop the mark.
  - `image` renders `<img>` with `srcset` from the asset variants, `sizes="(min-width: 768px) 720px, 100vw"`, `loading="lazy"` and `decoding="async"`.
  - `youtube` renders a facade: `<a class="yt-facade" href="https://www.youtube.com/watch?v={id}" data-video-id="{id}">` with a local placeholder image, and no iframe.
  - All text is HTML-escaped.

- [ ] **Step 1: Failing tests:**
  - `drops_unknown_nodes_keeps_text`.
  - `javascript_link_mark_removed`.
  - `escapes_text`.
  - `heading_level_1_becomes_2`.
  - `youtube_renders_facade_without_iframe`.
  - `invalid_video_id_dropped`.
  - `image_uses_variants_srcset`.
  - `pasted_word_styles_ignored` (a fixture with `textStyle` and `color` marks renders plain text).
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(api): validate and render rich text from an allowlist`.

### Task 9: Village profile editing and cache purge signal

**Decisions:** D-12, D-35, D-39

**Files:** Create `apps/api/src/village/profile.rs`, `src/cache_purge.rs`, `tests/village_profile.rs`

**Interfaces:**
- Consumes: `audit::record`, `jobs::enqueue`, `rich_text::{validate,to_html}`, `CurrentUser`, `MediaRef`.
- Produces:
  - `GET /api/admin/village/profile` → `{ overview: Option<RichTextDoc>, hero: Option<MediaRef>, updated_at }`.
  - `PUT /api/admin/village/profile { overview: Value|null, hero_asset_id: Uuid|null, updated_at }` → 200 with the same shape.
    - A stale `updated_at` gives 409 `stale_write`.
    - A hero asset that is not a ready image gives 422 `hero_not_ready`.
    - In one transaction: the update, an Audit Entry (`Update`, `object_type = "village"`, with a diff) and `enqueue(Job::PurgePublicCache)`.
  - `pub struct Purger { client: reqwest::Client, url: Url, token: SecretString }`, with `async fn purge_all(&self) -> Result<()>`. It sends `POST {web_internal_url}/_internal/cache/purge` with header `x-pranata-purge-token`; a non-2xx response is an error, so the job retries.
  - `VillageProfile` (Task 5) gains `overview_html: Option<String>` and `hero: Option<MediaRef>`, which `GET /api/public/village` now fills.

- [ ] **Step 1: Failing tests:**
  - `put_profile_updates_and_audits`.
  - `put_profile_enqueues_purge_in_same_tx` (a rollback leaves no job).
  - `stale_update_is_409`.
  - `pending_hero_is_422`.
  - `public_village_returns_rendered_html`.
  - `purge_job_retries_when_web_is_down` (point at a closed port; job `attempts == 1`, `run_at` in the future).
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(api): edit the village profile and signal public cache purges`.

### Task 10: Web foundation

**Decisions:** D-11, D-13, D-20, D-38, D-08, D-41

**Files:** Create `apps/web/{package.json,rsbuild.config.ts,orval.config.ts,components.json,tsconfig.json}`, `src/{router.tsx,styles.css}`, `src/routes/__root.tsx`, `src/components/ui/*`, `biome-plugins/no-raw-controls.grit`, `src/lib/errors.ts`. Modify `biome.json` and `justfile`.

**Interfaces:**
- Produces:
  - Rsbuild with `pluginReact()` and `tanstackStart()` from `@tanstack/react-start/plugin/rsbuild`. Path alias `@/` → `src/`.
  - Orval: input `../api/openapi.json`, output `src/api/` (client `react-query`, `httpClient: fetch`, base URL `/api`). A custom fetch mutator (`src/api/fetcher.ts`) sends `credentials: "include"` and the `x-csrf-token` header from `src/lib/csrf.ts` (`export function getCsrfToken(): string | null`, `export function setCsrfToken(t: string | null): void`). Task 12 sets the token after login and after `/api/auth/me`.
  - shadcn initialised with the Base UI variant and these components: button, input, label, textarea, select, dialog, sheet, sidebar, card, badge, tabs, table, dropdown-menu, separator, skeleton, sonner, form helpers (field).
  - `src/lib/errors.ts`: `export function messageFor(code: string): string` maps API codes to Indonesian copy and falls back to "Terjadi kesalahan. Coba lagi." Codes in R0: `invalid_credentials` → "Nama pengguna atau kata sandi salah.", `access_link_used` / `access_link_expired` → "Tautan ini sudah tidak berlaku. Minta tautan baru kepada Administrator.", `password_length` → "Kata sandi 10–128 karakter.", `unsupported_media_type` → "Format foto tidak didukung. Gunakan JPEG, PNG, atau WebP.", `file_too_large` → "Ukuran berkas melebihi batas.", `image_too_large` → "Resolusi foto terlalu besar.", `stale_write` → "Data sudah diubah orang lain. Muat ulang halaman.", `hero_not_ready` → "Foto masih diproses. Tunggu sebentar."
  - Theme in `styles.css`:
    - Public tokens follow `.prototype/public/site.css`: `--primary: #006D4B`, `--primary-dark: #00513A`, radius 4 px.
    - Admin tokens follow the shadcn neutral theme in `.prototype/admin/index.html`.
    - `--primary` is a CSS variable so Village Settings can override it in R1.
    - Geist via `@fontsource-variable/geist`.
  - Biome:
    - The plugin `biome-plugins/no-raw-controls.grit` matches JSX elements `button|input|select|textarea|dialog` with the message "Use components from @/components/ui (ADR 0008)", applied to `apps/web/src/**` except `src/components/ui/**`.
    - `noRestrictedImports` forbids `@/admin/*` and `@/routes/admin/*` in `src/routes/index.tsx`, `src/routes/masuk.tsx` and every public route file (override glob `src/routes/!(admin)/**`, plus top-level route files except `admin/`).

- [ ] **Step 1: Prove the lint rule.** Add a temporary `src/routes/_lint-probe.tsx` rendering `<button>` → `bunx biome ci` FAILS with the ADR 0008 message. Replace it with `<Button>` from `@/components/ui/button` → PASS. Delete the probe.
- [ ] **Step 2: Prove the import boundary.** In `src/routes/index.tsx` add `import "@/admin/x"` → FAIL. Remove it.
- [ ] **Step 3: Unit test** `src/lib/errors.test.ts`: `maps known codes` (two samples) and `falls back for unknown`. Run `bun run test` (Rstest) → PASS after implementation.
- [ ] **Step 4: Codegen and contract.** `just gen` produces `src/api/`. `just check-contract` fails when `apps/api/openapi.json` or `apps/web/src/api` differ after `just gen` (`git status --porcelain`). Verify by editing a generated file → FAIL, then reverting.
- [ ] **Step 5: Commit** in three commits: `feat(web): scaffold tanstack start on rsbuild with tailwind and shadcn`, `build(web): forbid raw form controls and admin imports in public routes`, `feat(web): generate the api client from openapi`.

### Task 11: Public home through the HTML cache

**Decisions:** D-11, D-12, D-36, D-09, D-20, D-58

**Files:** Create `apps/web/src/server/{entry.ts,html-cache.ts,csp.ts}`, `src/server/html-cache.test.ts`, `src/server/csp.test.ts`, `src/routes/index.tsx`, `src/components/site/{header,footer,hero}.tsx`

**Interfaces:**
- Produces:
  - `export function createHtmlCache(opts: { max: number; ttlMs: number; now?: () => number }): { get(key: string): string | undefined; set(key: string, html: string): void; purgeAll(): void; size(): number }`. It is an LRU; defaults are `max: 500` and `ttlMs: 300_000`.
  - `src/server/entry.ts` is a custom server entry (srvx) wrapping the Start handler:
    - Caching applies only to `GET` responses with status 200 for paths that do not start with `/admin`, `/masuk` or `/akses`, when the request carries no `pranata_session` cookie. The key is path + query.
    - On a cache hit it serves the stored HTML with a fresh nonce substituted.
    - `POST /_internal/cache/purge` with a valid `x-pranata-purge-token` (env `PRANATA_PURGE_TOKEN`) calls `purgeAll()` and returns 204; anything else returns 404.
  - `src/server/csp.ts`:
    - `export function nonce(): string` returns 16 random bytes in base64.
    - `export const NONCE_PLACEHOLDER = "__PRANATA_NONCE__"`.
    - `export function cspHeader(n: string): string` returns `default-src 'self'; script-src 'self' 'nonce-{n}'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-src https://www.youtube-nocookie.com; frame-ancestors 'none'; base-uri 'none'; form-action 'self'`.
    - The SSR render writes `NONCE_PLACEHOLDER` into script tags. The entry replaces it per response, both on a cache miss and on a hit.
  - Route `/`:
    - Loader: `getPublicVillage()` (Orval, server-side against `PRANATA_API_INTERNAL_URL`).
    - Renders site header (village name, "Kab. {regency}, {province}"), a hero (photo `srcset` from variants with `fetchpriority="high"`, overview HTML), and a footer ("© {year} Pemerintah Desa {name} · Dikelola dengan Pranata").
    - Before init it renders "Situs desa belum disiapkan." with status 503.
    - The layout follows `.prototype/public/index.html` (hero and footer only in R0). Other sections come in Plan R1-D (D-58).
  - Accessibility basics: `lang="id"`, a skip link "Lewati ke konten" targeting `#konten`, a landmark per region.

- [ ] **Step 1: Failing unit tests** (Rstest):
  - `html cache evicts least recently used`.
  - `html cache expires after ttl` (injected clock).
  - `purgeAll empties`.
  - `csp contains nonce and forbids framing`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(web): render the public home through a purged html cache`.

### Task 12: Admin sign-in, layout and profile editor

**Decisions:** D-06, D-16, D-17, D-23, D-38, D-32

**Files:** Create `apps/web/src/routes/masuk.tsx`, `routes/akses.tsx`, `routes/admin/route.tsx`, `routes/admin/index.tsx`, `routes/admin/desa/profil.tsx`, `src/admin/{layout.tsx,nav.ts,session.ts}`, `src/components/rich-text-editor.tsx`, `src/components/image-upload.tsx`, `src/lib/downscale-image.ts`

**Interfaces:**
- Consumes: the Orval hooks `useLogin`, `useGetMe`, `useSetPasswordWithAccessLink`, `useGetVillageProfile`, `usePutVillageProfile`, `useUploadMedia`, `useGetMedia`.
- Produces:
  - `admin/route.tsx` sets `ssr: false`. Its `beforeLoad` calls `/api/auth/me` and redirects to `/masuk?ke={path}` on 401.
  - `src/admin/nav.ts` holds the admin menu structure from `.prototype/admin/index.html` (groups Desa, Kependudukan, Surat, Situs web, Sistem). In R0 only "Beranda" and "Desa → Profil desa" are enabled. Groups for R2/R3 are not rendered at all (D-04 spirit: no dead menu).
  - `/masuk`:
    - Follows `.prototype/login/index.html`: fields "Nama pengguna" and "Kata sandi" with a show/hide toggle, and "Ingat saya di perangkat ini".
    - The "Lupa kata sandi?" link opens a dialog with "Hubungi Administrator desa untuk mendapatkan tautan akses baru." (ADR 0007).
    - The Passkey button is hidden until Plan R1-A.
    - The photo carousel uses a static illustration in R0.
  - `/akses` reads the token from `location.hash` (never from the path or query), then shows a set-password form (new password plus confirmation) that POSTs `{token, password}` to `/api/auth/access-links/redeem`. It clears the hash with `history.replaceState` after reading it. On success it redirects to `/masuk`.
  - `export async function downscaleImage(file: File, maxEdge = 2560, quality = 0.85): Promise<Blob>` uses `createImageBitmap(file, { imageOrientation: "from-image" })` and `OffscreenCanvas.convertToBlob({ type: "image/jpeg", quality })`. Files already within `maxEdge` and under 2 MB pass through unchanged. Non-images pass through, and the server decides.
  - `<RichTextEditor value onChange />`:
    - Tiptap StarterKit restricted to the D-17 allowlist (heading levels 2–4, the table extension, the YouTube extension storing only `videoId`, and image nodes inserted from uploads storing `assetId`).
    - Toolbar buttons use `ui/button`, following the prototype's editor toolbar.
    - Usable at 390 px width: the toolbar scrolls horizontally, and the content font is 16 px so iOS does not zoom.
  - `<ImageUpload onUploaded />` takes multiple files from camera or gallery (`accept="image/*"`, no `capture`), downscales each, uploads sequentially with a progress list, and polls `GET /api/admin/media/{id}` until it is `ready` or `failed`.
  - The profile page has a card "Gambaran umum" (editor) and a card "Foto hero" (upload or replace). The "Simpan" button sends `updated_at` for the 409 check. A save shows a sonner toast "Profil desa disimpan."
  - D-23: the profile is status-less content, so saving makes it live (D-23's status-less rule). Permission checks come in Plan R1-A.

- [ ] **Step 1:** Implement the routes and components above. They are exercised end to end in Task 14. Run `bun run typecheck` and `bunx biome ci` → PASS.
- [ ] **Step 2: Commit** in two commits: `feat(web): sign in and set a password from an access link` and `feat(web): edit the village profile with rich text and phone photo uploads`.

### Task 13: Container images

**Decisions:** D-48

**Files:** Create `apps/api/Containerfile`, `apps/web/Containerfile`, `.containerignore`. Modify `justfile`.

- [ ] **Step 1: API image.** A multi-stage build: `rust:1.100-trixie` builds with `SQLX_OFFLINE=true` and links libwebp statically (`webp` crate feature or vendored build). The runtime image is `gcr.io/distroless/cc-debian13:nonroot`, with entrypoint `["/pranata"]` and cmd `["serve"]`.
- [ ] **Step 2: Web image.** Build with `oven/bun:1.4`; the runtime image is the Bun distroless image (`oven/bun:1.4-distroless`), running as non-root with `CMD ["bun","dist/server/entry.js"]`.
- [ ] **Step 3: Recipe.** `just images` builds `pranata-api:local` and `pranata-web:local` for `linux/amd64`.
- [ ] **Step 4: Verify** `podman run --rm pranata-api:local --version` prints the version. `podman image inspect` shows `User` is non-root for both images.
- [ ] **Step 5: Commit** `build: add distroless non-root images for api and web`.

### Task 14: End-to-end, accessibility, performance and load gates

**Decisions:** D-40, D-09, D-08, D-07, D-06, D-16, D-20, D-36, D-12, D-55

**Files:** Create `apps/web/playwright.config.ts`, `apps/web/e2e/{stack.ts,public-home.spec.ts,admin-profile.spec.ts,fixtures/gps-photo.jpg}`, `apps/web/lighthouserc.cjs`, `apps/web/.size-limit.cjs`, `load/public.js`, `infra/dev/stack.sh`, `infra/dev/Caddyfile`. Modify `justfile`.

**Interfaces:**
- `infra/dev/stack.sh up|down` runs the production images in a Podman pod `pranata-e2e` created with `--cpus=2 --memory=2g`, containing postgres:19, api, web and Caddy. Caddy uses `infra/dev/Caddyfile` (created here) with internal TLS and the same routes Task 16's `Caddyfile.j2` must use: `/api/*` → api, `/media/*` → files with immutable caching, `/_internal/*` → 404, everything else → web. It runs `pranata village init --code <fixture code>` and `pranata user create-admin e2e --name "Admin E2E"`, and writes the Access Link to `target/e2e/access-link.txt`.
- `just e2e` = `images` + `stack up` + `bunx playwright test` + `stack down`.
- `just perf` = Lighthouse CI against the running stack.
- `just load` = `k6 run load/public.js`.

- [ ] **Step 1: `public-home.spec.ts`:**
  - `renders content without javascript` (JS disabled: the village name and overview are present).
  - `makes no third-party requests` (all requests have the stack's origin).
  - `has no axe violations` (`@axe-core/playwright`, tags `wcag2a, wcag2aa, wcag21aa, wcag22aa`).
  - `sends csp with a fresh nonce per response` (two requests, different nonces, and the nonce in the HTML matches the header).
  - `public js budget` (sum of gzip-transferred `script` responses on `/` ≤ 120 KB).
  - `shows unprepared message before init` runs against a fresh pod variant (stack.sh `--no-init`), expecting status 503.
- [ ] **Step 2: `admin-profile.spec.ts`** at viewport 390×844:
  - `access link → set password → sign in`.
  - `edit overview with tiptap and save`, then an anonymous context loads `/` and sees the new text immediately (proves the purge).
  - `upload phone photo` with `fixtures/gps-photo.jpg` (8 MP with GPS EXIF): it becomes ready, the hero shows on `/`, and fetching the largest variant URL and parsing it shows no EXIF (use `exifr` in the test).
  - `heic upload shows indonesian error` ("Format foto tidak didukung…").
  - `admin pages have no axe violations`.
  - `second use of access link shows expiry message`.
- [ ] **Step 3: Lighthouse CI.** `lighthouserc.cjs` uses the mobile preset on `/` with the assertion `categories:performance >= 0.9`, plus `accessibility >= 0.95`.
- [ ] **Step 4: size-limit.** A limit on the public entry chunk (`dist/client/assets/index-*.js`) of `120 KB` gzip.
- [ ] **Step 5: k6.** `load/public.js` ramps over 30 s to 300 VUs, holds 2 min, then ramps down over 15 s. Each VU requests `/` and one hero variant, with think time 1–3 s. Thresholds: `http_req_duration{expected_response:true}: ["p(95)<1000"]`, `http_req_failed: ["rate<0.01"]`. R1 plans extend the scenario with news, gallery and search (D-07).
- [ ] **Step 6: Run** `just e2e`, `just perf` and `just load` locally → all pass. If load fails, profile and fix before continuing (ADR 0003 fallback only after a written analysis in the PR).
- [ ] **Step 7: Commit** in three commits: `test(web): add end-to-end and accessibility checks against the images`, `test(web): gate lighthouse and bundle size`, `test: gate public pages under load with k6`.

### Task 15: Continuous integration and releases

**Decisions:** D-45, D-41, D-44, D-46, D-13, D-42, D-02, D-60

**Files:** Create `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `deny.toml`, `renovate.json`, `release-please-config.json`, `.release-please-manifest.json`

- [ ] **Step 1: `ci.yml`** runs on `pull_request` and `push: main`, with every action pinned to a full SHA and a version comment. Jobs, each calling `just` recipes only:
  - `commits`: `just check-commits origin/main`.
  - `lint`: Biome and `cargo fmt --check`, clippy, `just check-village-data`, cargo-deny `check advisories licenses bans`.
  - `typecheck`.
  - `test`: Postgres 19 service with the init SQL, then `just test`.
  - `contract`.
  - `trace`: `just check-trace`.
  - `infra-lint` and `infra-test`, added to the workflow by Tasks 16 and 17 when they land.
  - `build`: `just images`, with the images uploaded as an artifact.
  - `e2e`, `perf` and `load`: these download the images, run on `ubuntu-latest` (4 vCPU for public repos), and pass the pod limits from Task 14.
- [ ] **Step 2: `deny.toml`.** Allowed licences: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0, AGPL-3.0-or-later, and the libwebp licence (BSD-3-Clause). Any other licence fails the job.
- [ ] **Step 3: Releases.**
  - `release.yml`: release-please (`release-type: simple`, one version for the repository, CHANGELOG at the root). On release created, build both images and push them to `ghcr.io/dodoarmando/pranata-api:{version}` and `ghcr.io/dodoarmando/pranata-web:{version}`, plus `:latest`.
  - The workflows contain no deploy job and no SSH secret (D-60).
  - `renovate.json`: `config:recommended`, `helpers:pinGitHubActionDigests`, and grouping for the tanstack, tiptap and shadcn/base-ui packages.
- [ ] **Step 4: Branch protection on `main`** (maintainer confirms): require PRs, require all CI jobs above, require linear history, disallow force pushes. Verify with `gh api repos/dodoarmando/pranata/branches/main/protection`.
- [ ] **Step 5: Commit** in three commits: `ci: run every gate on pull requests with pinned actions`, `ci: release versioned images with release-please`, `build(deps): let renovate keep dependencies current`.

### Task 16: Provisioning one Installation with Ansible

**Decisions:** D-48, D-49, D-60, D-01, D-50

**Files:** Create `infra/{provision.yml,deploy.yml,inventory.example.yml,requirements.yml,requirements.txt,.ansible-lint}`, `infra/roles/pranata/{defaults/main.yml,handlers/main.yml,tasks/{main,base,ssh,firewall,podman,app}.yml,templates/{pranata.network,pranata-postgres.container,pranata-api.container,pranata-web.container,pranata-caddy.container,Caddyfile.j2,api.env.j2,web.env.j2,nftables.conf.j2}}`, `infra/test/Containerfile`. Modify `justfile`.

**Interfaces:**
- Inventory variables (`defaults/main.yml`):
  - `pranata_domain`.
  - `pranata_version`, which must be set.
  - `pranata_tls`: `acme` (default) or `internal` (test).
  - `pranata_admin_username`, `pranata_admin_name`.
  - `pranata_village_code`.
  - Secrets: `pranata_db_owner_password`, `pranata_db_app_password`, `pranata_purge_token`, `pranata_restic_password`, `pranata_restic_repository`, `pranata_restic_env` (S3 keys).
  - Optional: `pranata_monitor_ping_urls`, a dict with keys `backup`, `restore`, `disk`.
  - Real inventories live outside the repo; the operator points `PRANATA_INVENTORY` at theirs.
- Behaviour:
  - Base: `apt` upgrade, `unattended-upgrades` enabled for security updates, `podman`, `restic`, `nftables`.
  - SSH: password auth off, root login off. Firewall: allow 22, 80 and 443 (tag `firewall`).
  - A system user `pranata` with linger. Quadlet units go in `~pranata/.config/containers/systemd/`, on one network `pranata`.
  - The API container mounts the media volume. Caddy mounts `media/public` read-only at `/media`.
  - Caddyfile:
    - `/api/*` → `api:8080`; `/media/*` → `file_server` with `Cache-Control: public, max-age=31536000, immutable`.
    - `/_internal/*` → `respond 404`.
    - Everything else → `web:3000`.
    - `encode zstd gzip`.
  - The first run (when no Village exists) runs `pranata village init` and `pranata user create-admin`, and prints the Access Link once as Ansible debug output.
  - `deploy.yml` only changes `pranata_version`, pulls the images, restarts `api` (migrations run on start) then `web`, and waits for `/api/healthz` to report the new version. A failed health check rolls back automatically to the previous version tag recorded in `~pranata/.pranata-version`. The N/N-1 schema compatibility test is Plan R1-E.
- Recipes:
  - `just deploy host version` = `ansible-playbook -i $PRANATA_INVENTORY infra/deploy.yml -l {{host}} -e pranata_version={{version}}`; with `host=all` it targets every Installation.
  - `just provision host`.
  - `just check-infra` = ansible-lint, `ansible-playbook --syntax-check`, `podman-system-generator --dryrun` against the rendered templates.

- [ ] **Step 1: `infra-test`.** `infra/test/Containerfile` builds a privileged Debian 13 image with systemd. The CI job runs `provision.yml` against it with `pranata_tls=internal` and `--skip-tags firewall,ssh`, using the `local` images loaded from the build artifact. It asserts `curl -k https://localhost/api/healthz` returns the version, `/_internal/cache/purge` returns 404 through Caddy, and `/media/` returns immutable headers.
- [ ] **Step 2: Run** `just check-infra` and the infra test locally → PASS.
- [ ] **Step 3: Commit** in two commits: `feat(infra): provision an installation with rootless podman and caddy` and `feat(infra): deploy a version with automatic rollback on failed health`.

### Task 17: Encrypted backups, restore check and monitoring

**Decisions:** D-51, D-52, D-53

**Files:** Create `infra/roles/pranata/tasks/backup.yml`, `tasks/monitoring.yml`, `templates/{pranata-backup.service,pranata-backup.timer,pranata-restore-check.service,pranata-restore-check.timer,pranata-disk-check.service,pranata-disk-check.timer,backup.sh.j2,restore-check.sh.j2,disk-check.sh.j2}`. Modify `infra/test/` and the CI `infra-test` job.

**Interfaces:**
- `backup.sh`:
  1. `podman exec pranata-postgres pg_dump -Fc` into a temp file.
  2. `restic backup` of the dump and the media `originals/` directory, tagged `pranata`.
  3. `restic forget --keep-daily 7 --keep-weekly 4 --keep-monthly 12 --prune`.
  4. Ping `pranata_monitor_ping_urls.backup`, adding `/fail` on error.
  - The timer runs daily at 02:00 local time, with `RandomizedDelaySec=30m`.
  - Variants are not backed up; they can be regenerated from originals.
- `restore-check.sh` runs monthly. It restores the latest dump into the scratch database `pranata_restore_check`, runs `SELECT count(*) FROM village`, drops the database, and pings `restore`.
- `disk-check.sh` runs hourly. It pings `disk`, with `/fail` when `/` usage exceeds 80 %.
- The ping URL format is Healthchecks.io-compatible (self-hostable). The operator chooses the service; see `infra/README.md`.
- No LUKS/Clevis/Tang (D-53, ADR 0005).

- [ ] **Step 1: Extend `infra-test`.** Run an S3-compatible server in CI (Garage; re-check its latest version), then run backup, delete a row, run restore into the scratch database and assert the row exists there. Then run `restic snapshots --json` and assert one snapshot is tagged `pranata`.
- [ ] **Step 2: Run** locally → PASS.
- [ ] **Step 3: Commit** `feat(infra): back up to object storage with monthly restore checks and pings`.

### Task 18: Staging Installation and R0 exit

**Decisions:** D-59, D-55, D-54

- [ ] **Step 1: Procure (maintainer).** `infra/README.md` gets a "Before you start" section listing exactly what to buy:
  - A VPS with Debian 13, 2 vCPU, 2 GB RAM and ≥ 40 GB disk, in Indonesia where available.
  - A domain or subdomain (for staging, for example `staging.<maintainer domain>`).
  - An S3-compatible bucket, preferably in Indonesia, with an access key limited to that bucket.
  - The maintainer procures these and fills a private inventory from `inventory.example.yml`.
- [ ] **Step 2: First release.** Merge a `feat` change so release-please opens `0.1.0`. Merge it, and confirm the GHCR images for `0.1.0` exist (`gh api /users/dodoarmando/packages/container/pranata-api/versions`).
- [ ] **Step 3: Provision staging.** `just provision staging` → the Access Link is printed. Open it, set a password, sign in, edit the profile and upload a photo from a real phone, then confirm the public home updates.
- [ ] **Step 4: Backups on staging.** Run `systemctl --user start pranata-backup` as `pranata` → a snapshot exists in the bucket, and the backup ping is received.
- [ ] **Step 5: Draft the install guide.** `infra/README.md` covers procurement, inventory, provision, first Administrator, deploy, rollback, backups and restore. D-54's stranger test runs before R1 (Plan R1-E).
- [ ] **Step 6: R0 exit checklist.** Write it in the PR that closes R0 and tick every item:
  - All CI jobs are required and green on `main`.
  - `just check-trace` is green.
  - The D-38 lint rule is proven (Task 10 Step 1).
  - Staging is up on version `0.1.x`.
  - A backup snapshot is present.
  - The load gate passes.
  - Lighthouse is ≥ 90.
- [ ] **Step 7: Commit** `docs(infra): write the installation guide for one village`.

---

## R1 roadmap (plans written after R0 lands)

Plans are named after their release. This plan is R0 (the walking skeleton). The R1 plans are R1-A to R1-E, in the order below. Each plan below is written with the writing-plans skill once R0's code exists, so its file paths and interfaces are exact. A decision listed here is considered planned, not built; `just check-trace-strict`, which gates the R1 release, requires each one to appear on a task's `**Decisions:**` line.

| Plan | Scope | Decisions |
|---|---|---|
| R1-A Access | Roles and the fixed Permission catalogue (including `budget.manage`), passkeys, sudo mode, login throttling, Users screen, Roles screen, Audit Log screen, Access Link issuing from the admin, login carousel | D-30, D-31, D-32, D-33, D-34, D-35, D-21, D-38, D-39 |
| R1-B Village | Setup Wizard, Village Settings (logo, colour, Hamlet naming, office hours, contacts, social links, tile source, external service URL), region tree, Positions/Tenures/Officials, Institutions, Indeks Desa Editions, map with Landmarks and boundary | D-02, D-21, D-22, D-24, D-29, D-57, D-39, D-06 |
| R1-C Website content | Publication workflow, News Posts, Announcements, Events, Pages, Albums, Important Contacts and Contact Categories, Partners, Transparency Documents, APBDes Summaries, Quick Links, Categories, Home Hero, Head's Welcome, search API, slugs and redirects, dashboard | D-23, D-25, D-26, D-27, D-28, D-17, D-18, D-24, D-12, D-14, D-39 |
| R1-D Public site | Every public page of D-03, home sections in order, hidden sections and the external service button, accessibility panel, SEO (sitemap, RSS, structured data, Open Graph), 404, mobile-first layouts, extended k6 scenario | D-03, D-04, D-06, D-07, D-08, D-09, D-19, D-20, D-58, D-57, D-11 |
| R1-E Operations and release | N/N-1 migration compatibility test, multi-Installation deploy, monitoring docs, demo seed, the stranger install test, R1 release checklist with strict trace | D-50, D-51, D-52, D-53, D-54, D-56, D-01, D-46, D-55, D-10, D-15, D-16, D-36, D-37, D-40, D-41, D-42, D-43, D-44, D-45, D-47, D-48, D-49, D-59, D-60, D-13 |

**Not built:** D-05 (WordPress import is a non-goal).

---

## Carry-over into R1 plans

The final whole-branch review of R0 deferred these items, and each later plan adds here what it defers to another plan. Each plan must pick up its group when it is written. Items are one line each, with their source.

### Carry-over to the Access plan (R1-A)

- `audit::fields_of` silently maps non-object values to an empty diff (final review).
- An old session row survives re-login (final review).
- Deactivated users can still redeem Access Links, and their other links stay valid (final review).
- No concurrency test for double redemption of an Access Link (final review).
- Malformed JSON gets a text/plain rejection instead of a problem document (final review).
- A password over 128 characters is dummy-verified on the full input (final review).
- There is no way to issue an Access Link for an existing user (final review).
- Per-IP throttling sees a single IP behind rootless port forwarding; use pasta source-IP preservation or a trusted X-Forwarded-For from Caddy (final review).

### Carry-over to the Village plan (R1-B)

- VillageProfile construction is duplicated (final review).
- Dataset currency: the README says 2430 while the SQL header says 2138; verify against the official Kepmendagri (final review).
- Large transparent PNGs become JPEGs with a black background (final review).
- The Users screen has no "Perangkat desa" column linking a User to an Official, as the admin prototype shows; add it once Officials exist (R1-A plan, Task 10).

### Carry-over to the Website content plan (R1-C)

- An orphan file is left when the upload transaction rolls back; add media garbage collection (final review).
- ICC colour profiles are dropped (final review).
- PDFs need public serving (final review).
- Revisit the fixed 300 s job lease (final review).
- `process_asset` ready and reject updates lack a `status = 'pending'` guard (final review).
- There is no structural schema check for rich text (final review).
- The YouTube anchor lacks a rel attribute (final review).
- `is_media_url` accepts `..` (final review).
- A no-op PUT writes an audit entry and purges the cache (final review).
- Upload polling swallows network errors (final review).
- An imperative upload that gets a 401 bypasses the central redirect (final review).
- Weak-signal uploads have no timeout and show only a fake 45 % progress; use XHR progress plus a timeout (final review).
- Session expiry loses edits; keep a local draft (final review).
- D-31: deleting published content must require Sudo Mode through `AuthSession::require_sudo`, with a test per delete route (R1-A plan, Task 4).
- The sign-in carousel shows built-in slides; switch it to photos from published Albums (R1-A plan, Task 15).

### Carry-over to the Public site plan (R1-D)

- API down gives a 500 with the English Start error page; add stale-if-error and an Indonesian error page (final review).
- The SSR fetch has no timeout; set 5 s (final review).
- The purge e2e should assert `x-cache: HIT` before the save (final review).
- The k6 scenario only exercises a warm cache; add misses and purges (final review).
- Cache-busting query churn (final review).
- A cache HIT drops the Link and Cache-Control headers (final review).
- Facade: focus handling and modifier-click (final review).
- Render the uppercase eyebrow via CSS (final review).
- The 503 page lacks a skip link and header (final review).
- No test for a throwing leader render (final review).
- SSR cookie forwarding is needed once SSR is authenticated (final review).
- The modulepreload count is not captured (final review).
- The e2e tests depend on execution order (final review).

### Carry-over to the Operations and release plan (R1-E)

- The trace parser ignores `### Task N` without a colon, ignores fenced blocks, and reads D-1234 as D-123; fix this before `check-trace-strict` gates R1 (final review).
- `migrate` and `village init` need unrelated config (final review).
- `x-request-id` is not validated or bounded (final review).
- The image has no numeric USER, no `EXPOSE 8080` and no OCI labels (final review).
- The release workflow rebuilds images instead of publishing the ones CI tested (final review).
- `.pranata-version` is never read, and there is no `just rollback` (final review).
- The superuser password is passed via `Environment=` and derived from other secrets (final review).
- psql passwords appear in argv (final review).
- Restarts use loop-index logic (`results[3]`) (final review).
- The sysctl tolerance is tied to the TLS mode (final review).
- A single network is shared by db, web and caddy (final review).
- Caddy needs hardening (final review).
- `postgres:19` floats (final review).
- No DNS pre-check before ACME (final review).
- Some CI steps run outside `just` (final review).
- Untested: the ssh happy path, the ROLLBACK FAILED rescue, the nft check in check-infra, and the 120 s TimeoutStartSec against long migrations (final review).
- No retention test for the backups (final review).
- The API serves with owner DB credentials; run `migrate` as ExecStartPre with the owner URL and give `serve` the app URL only (final review).
- Docs nits for the stranger install test (final review):
  - add a journalctl `_UID` fallback;
  - note the Access Link appears inside the debug msg;
  - mention the 10–128 character password rule;
  - fix the adduser flag ambiguity;
  - use `sudo -H`;
  - explain the vault flag;
  - list the sudo and python3 prerequisites;
  - rehearse the disaster restore.
- Remove `release-as` from `release-please-config.json` after the 0.1.0 release (final fix wave).
- Two HIGH findings with no fix in the api base image (libssl3t64, CVE-2026-107494 and CVE-2026-124709) are skipped by `just scan-images`; re-check them when the base image updates (final fix wave).
- A job that exhausts its retries on a transient error leaves the raw upload on disk; only a permanent rejection deletes it (final fix wave).
- Caddy runs with host networking and the api and web containers publish on 127.0.0.1 only (ADR 0009, R1-A plan, Task 6); cover this in the install guide, the stranger install test and the multi-Installation deploy (R1-A plan).
- Rolling back to 0.1.0 breaks only `pranata user create-admin`, because `users.role_id` is NOT NULL; the N/N-1 compatibility test must list this as its one accepted exception (R1-A plan, Task 3).
