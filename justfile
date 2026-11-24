set shell := ["bash", "-euo", "pipefail", "-c"]

# Dev defaults for sqlx macros and tests (trust auth, dev container only).
export DATABASE_URL := env("DATABASE_URL", "postgres://postgres@127.0.0.1:55432/pranata")

# List available recipes.
default:
    @just --list

# Install dependencies (Task 10).
install:
    @echo "install: not implemented yet (Task 10)"

# Run the API and web app in development mode (Task 10).
dev:
    @echo "dev: not implemented yet (Task 10)"

# Start the local database container (no-op in CI, where a service container is used).
db-up:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "${CI:-}" ]; then
        echo "db-up: CI is set, skipping (CI provides its own postgres)"
        exit 0
    fi
    if podman container exists pranata-db; then
        podman start pranata-db >/dev/null
    else
        podman run -d --name pranata-db \
            -e POSTGRES_HOST_AUTH_METHOD=trust \
            -e POSTGRES_DB=pranata \
            -p 127.0.0.1:55432:5432 \
            -v "{{ justfile_directory() }}/infra/dev/postgres-init.sql:/docker-entrypoint-initdb.d/00-init.sql:ro,z" \
            docker.io/library/postgres:19 >/dev/null
    fi
    until podman exec pranata-db pg_isready -U postgres -d pranata -h 127.0.0.1 >/dev/null 2>&1; do
        sleep 1
    done
    echo "db-up: postgres ready on 127.0.0.1:55432"

# Apply database migrations with the owner role (sqlx-cli, so it works before the crate compiles).
db-migrate:
    sqlx migrate run --source apps/api/migrations --database-url "${MIGRATION_DATABASE_URL:-postgres://pranata_owner:pranata_owner_dev@127.0.0.1:55432/pranata}"

# Generate the OpenAPI contract (the web client codegen arrives in Task 10).
gen:
    cargo run -q -- openapi | bunx biome format --stdin-file-path=apps/api/openapi.json > apps/api/openapi.json.tmp
    mv apps/api/openapi.json.tmp apps/api/openapi.json
    @echo "gen: web client codegen (Orval) is not wired yet, it arrives in Task 10"

# Run all tests (the web tests join in Task 10).
test: test-api

# Run every check available so far (web and infra checks join in later tasks).
check: check-api check-trace check-village-data

# Validate commit messages from a ref to HEAD.
check-commits from="origin/main":
    bunx commitlint --from {{ from }} --to HEAD

# Run API checks: format, clippy pedantic, sqlx offline data, tests.
check-api:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo sqlx prepare --workspace --check -- --all-targets
    cargo test --workspace --locked

# Run web checks (Task 10).
check-web:
    @echo "check-web: not implemented yet (Task 10)"

# Verify the OpenAPI contract is up to date (Task 10).
check-contract:
    @echo "check-contract: not implemented yet (Task 10)"

# Verify every design decision is cited by a plan task, the roadmap or Not built.
check-trace:
    bun tools/trace/trace.ts

# Validate infrastructure files (Task 16).
check-infra:
    @echo "check-infra: not implemented yet (Task 16)"

# Run decision traceability in strict mode (roadmap-only citations fail).
check-trace-strict:
    bun tools/trace/trace.ts --strict

# Run API tests (needs `just db-up`).
test-api:
    cargo test --workspace --locked

# Provision one Installation (Task 16).
provision:
    @echo "provision: not implemented yet (Task 16)"

# Fail when real village data leaks outside allowed paths.
check-village-data:
    tools/no-village-data.sh

# Build container images (Task 13).
images:
    @echo "images: not implemented yet (Task 13)"

# Run end-to-end tests (Task 14).
e2e:
    @echo "e2e: not implemented yet (Task 14)"

# Run performance checks (Task 14).
perf:
    @echo "perf: not implemented yet (Task 14)"

# Run load tests (Task 14).
load:
    @echo "load: not implemented yet (Task 14)"

# Load seed data (plan P6).
seed:
    @echo "seed: not implemented yet (plan P6)"

# Deploy the stack (Task 16).
deploy:
    @echo "deploy: not implemented yet (Task 16)"
