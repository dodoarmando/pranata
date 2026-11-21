# Same URL locally (Podman) and in CI (service container).
export DATABASE_URL := env('DATABASE_URL', 'postgres://postgres@127.0.0.1:55432/pranata')
# Where the browser reaches the web app; `localhost`, not 127.0.0.1, for VS Code port forwarding.
export PRANATA_PUBLIC_ORIGIN := env('PRANATA_PUBLIC_ORIGIN', 'http://localhost:3000')
# Uploaded files in development, kept out of git.
export PRANATA_FILES_DIR := env('PRANATA_FILES_DIR', 'target/files')

db_container := 'pranata-db'

# List recipes
default:
    @just --list

# Install JS dependencies and git hooks
install:
    bun install

# Run Postgres, the API and the web dev server together
dev: install db-migrate
    #!/usr/bin/env bash
    trap 'kill 0' EXIT
    cargo run -p pranata -- serve &
    (cd apps/web && bun run dev) &
    wait

# Start the dev Postgres 18 container (no-op in CI, which provides its own)
db-up:
    #!/usr/bin/env bash
    set -euo pipefail
    [ -n "${CI:-}" ] && exit 0
    podman start {{ db_container }} >/dev/null 2>&1 || podman run -d --name {{ db_container }} \
        -p 127.0.0.1:55432:5432 -e POSTGRES_DB=pranata -e POSTGRES_HOST_AUTH_METHOD=trust \
        docker.io/library/postgres:18 >/dev/null
    until podman exec {{ db_container }} pg_isready -q -d pranata; do sleep 0.5; done

# Apply pending migrations; sqlx-cli, since checked queries cannot compile against an unmigrated database
db-migrate: db-up
    sqlx migrate run --source apps/api/migrations

# Regenerate the committed OpenAPI spec and the web client
gen:
    cargo run -q -p pranata -- openapi > apps/api/openapi.json
    cd apps/web && bun run gen

# Every required CI job except perf
check: check-commits check-api check-web check-contract images e2e

# Run all tests
test: test-api test-web

test-api: db-migrate
    cargo test --workspace --locked

test-web:
    cd apps/web && bun run test

# CI job `commits`: conventional messages, allowed scopes, DCO sign-off
check-commits from='origin/main':
    bunx commitlint --from {{ from }} --to HEAD --verbose

# CI job `api`
check-api: db-migrate
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo sqlx prepare --workspace --check
    cargo test --workspace --locked

# CI job `web`
check-web:
    bunx biome ci --error-on-warnings .
    cd apps/web && bun run typecheck && bun run test && bun run build && bun run size

# CI job `contract`: the committed spec and client match the code
check-contract: gen
    test -z "$(git status --porcelain -- apps/api/openapi.json apps/web/src/api)" \
        || { git status --short -- apps/api/openapi.json apps/web/src/api; echo 'Run `just gen` and commit the result.'; exit 1; }

# CI job `images`: build both production images (amd64), tagged `local`
images:
    podman build --platform linux/amd64 -f apps/api/Containerfile -t pranata-api:local .
    podman build --platform linux/amd64 -f apps/web/Containerfile -t pranata-web:local .

# Playwright against the production images and Postgres 18
e2e: images e2e-run

# CI job `e2e`: run the e2e tests on images already built (CI loads them from `images`)
e2e-run: db-up
    #!/usr/bin/env bash
    set -euo pipefail
    # Host network reaches the database on 127.0.0.1; ports differ from `just dev`.
    # The origin is `localhost`: passkeys cannot be bound to an IP address.
    api=(--network host -e DATABASE_URL -e PRANATA_LISTEN=127.0.0.1:18080 -e PRANATA_PUBLIC_ORIGIN=http://localhost:13000)
    web=(--network host -e HOST=127.0.0.1 -e PORT=13000 -e PRANATA_API_URL=http://127.0.0.1:18080 -e PRANATA_PUBLIC_ORIGIN=http://localhost:13000)
    cleanup() {
        if [ "$1" -ne 0 ]; then podman logs pranata-e2e-api; podman logs pranata-e2e-web; fi || true
        podman rm -fi -t 0 pranata-e2e-api pranata-e2e-web >/dev/null
    }
    trap 'cleanup $?' EXIT
    podman run --rm "${api[@]}" pranata-api:local migrate
    podman run --rm "${api[@]}" pranata-api:local village set-code 1101012001
    # The CLI prints the Access Link on stdout, as at provisioning. A fresh username each run,
    # since the local database outlives runs and usernames are never reused.
    export PRANATA_E2E_USERNAME="e2e$(date +%s)"
    PRANATA_E2E_ACCESS_LINK=$(podman run --rm "${api[@]}" pranata-api:local user create --admin "$PRANATA_E2E_USERNAME")
    export PRANATA_E2E_ACCESS_LINK
    podman run -d --name pranata-e2e-api "${api[@]}" pranata-api:local
    podman run -d --name pranata-e2e-web "${web[@]}" pranata-web:local
    timeout 60 bash -c 'until curl -sfo /dev/null http://127.0.0.1:18080/healthz && curl -sfo /dev/null http://127.0.0.1:13000/; do sleep 0.5; done'
    cd apps/web
    bunx playwright install chromium
    bun run e2e
