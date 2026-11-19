# Same URL locally (Podman) and in CI (service container).
export DATABASE_URL := env('DATABASE_URL', 'postgres://postgres@127.0.0.1:55432/pranata')

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

# Apply pending migrations to the dev database
db-migrate: db-up
    cargo run -q -p pranata -- migrate

# Regenerate the committed OpenAPI spec and the web client
gen:
    cargo run -q -p pranata -- openapi > apps/api/openapi.json
    cd apps/web && bun run gen

# Every required CI job except perf
check: check-commits check-api check-web check-contract images e2e

# Run all tests
test: test-api test-web

test-api: db-up
    cargo test --workspace --locked

test-web:
    cd apps/web && bun run test

# CI job `commits`: conventional messages, allowed scopes, DCO sign-off
check-commits from='origin/main':
    bunx commitlint --from {{ from }} --to HEAD --verbose

# CI job `api`
check-api: db-up
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo sqlx prepare --workspace --check
    cargo test --workspace --locked

# CI job `web`
check-web:
    bunx biome ci --error-on-warnings .
    cd apps/web && bun run typecheck && bun run test && bun run build

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
    api=(--network host -e DATABASE_URL -e PRANATA_LISTEN=127.0.0.1:18080)
    web=(--network host -e HOST=127.0.0.1 -e PORT=13000 -e PRANATA_API_URL=http://127.0.0.1:18080)
    cleanup() {
        if [ "$1" -ne 0 ]; then podman logs pranata-e2e-api; podman logs pranata-e2e-web; fi || true
        podman rm -fi -t 0 pranata-e2e-api pranata-e2e-web >/dev/null
    }
    trap 'cleanup $?' EXIT
    podman run --rm "${api[@]}" pranata-api:local migrate
    podman run -d --name pranata-e2e-api "${api[@]}" pranata-api:local
    podman run -d --name pranata-e2e-web "${web[@]}" pranata-web:local
    timeout 60 bash -c 'until curl -sfo /dev/null http://127.0.0.1:18080/healthz && curl -sfo /dev/null http://127.0.0.1:13000/; do sleep 0.5; done'
    cd apps/web
    bunx playwright install chromium
    bun run e2e
