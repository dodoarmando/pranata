set shell := ["bash", "-euo", "pipefail", "-c"]

# Dev defaults for sqlx macros and tests (trust auth, dev container only).
export DATABASE_URL := env("DATABASE_URL", "postgres://postgres@127.0.0.1:55432/pranata")

# List available recipes.
default:
    @just --list

# Install JavaScript dependencies (tooling, git hooks and the web app).
install:
    bun install --frozen-lockfile

# Run the API and the web dev server together (needs `just db-up db-migrate`).
# The web dev server proxies /api to the API. Values here are dev-only.
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    export PRANATA_DATABASE_URL="${PRANATA_DATABASE_URL:-postgres://pranata_app:pranata_app_dev@127.0.0.1:55432/pranata}"
    export PRANATA_MIGRATION_DATABASE_URL="${PRANATA_MIGRATION_DATABASE_URL:-postgres://pranata_owner:pranata_owner_dev@127.0.0.1:55432/pranata}"
    export PRANATA_PUBLIC_ORIGIN="${PRANATA_PUBLIC_ORIGIN:-http://localhost:3000}"
    export PRANATA_WEB_INTERNAL_URL="${PRANATA_WEB_INTERNAL_URL:-http://127.0.0.1:3000}"
    export PRANATA_PURGE_TOKEN="${PRANATA_PURGE_TOKEN:-dev-purge-token}"
    export PRANATA_MEDIA_DIR="${PRANATA_MEDIA_DIR:-{{ justfile_directory() }}/target/dev-media}"
    export PRANATA_LISTEN="${PRANATA_LISTEN:-127.0.0.1:8080}"
    mkdir -p "$PRANATA_MEDIA_DIR"
    trap 'kill 0' EXIT
    cargo run -q -- serve &
    bun run --filter @pranata/web dev &
    wait -n

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

# Apply database migrations with the owner role. The default URL holds the
# dev-only owner password from infra/dev/postgres-init.sql; real installations
# set MIGRATION_DATABASE_URL. (sqlx-cli, so it works before the crate compiles).
db-migrate:
    sqlx migrate run --source apps/api/migrations --database-url "${MIGRATION_DATABASE_URL:-postgres://pranata_owner:pranata_owner_dev@127.0.0.1:55432/pranata}"

# Generate the OpenAPI contract, then the web API client from it (Orval).
gen:
    SQLX_OFFLINE=true cargo run -q -- openapi | bunx biome format --stdin-file-path=apps/api/openapi.json > apps/api/openapi.json.tmp
    mv apps/api/openapi.json.tmp apps/api/openapi.json
    bun run --filter @pranata/web gen

# Run all tests.
test: test-api test-web

# Run every check available so far (infra checks join in later tasks).
check: check-api check-web check-trace check-village-data

# Validate commit messages from a ref to HEAD.
check-commits from="origin/main":
    bunx commitlint --from {{ from }} --to HEAD

# Lint without a database: Biome, rustfmt, clippy on the offline sqlx data, village data.
lint:
    bunx biome ci
    cargo fmt --all --check
    SQLX_OFFLINE=true cargo clippy --workspace --all-targets --locked -- -D warnings
    tools/no-village-data.sh

# Verify the committed sqlx offline data matches the queries (needs a migrated database).
check-sqlx:
    cargo sqlx prepare --workspace --check -- --all-targets

# Check dependency advisories, licences, bans and sources (deny.toml).
deny:
    cargo deny check advisories licenses bans sources

# Verify every release-managed version file agrees with .release-please-manifest.json.
check-versions:
    tools/ci/check-versions.sh

# Typecheck the web app.
typecheck:
    bun run --filter @pranata/web typecheck

# Run API checks: format, clippy pedantic, sqlx offline data, tests.
check-api:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo sqlx prepare --workspace --check -- --all-targets
    cargo test --workspace --locked

# Run web checks: Biome, typecheck, unit tests, production build.
check-web:
    bunx biome ci
    bun run --filter @pranata/web typecheck
    bun run --filter @pranata/web test
    bun run --filter @pranata/web build

# Verify the committed OpenAPI contract and generated web client are up to date.
check-contract:
    #!/usr/bin/env bash
    set -euo pipefail
    just gen
    paths=(apps/api/openapi.json apps/web/src/api/generated)
    if [ -n "$(git status --porcelain -- "${paths[@]}")" ]; then
        echo "check-contract: generated output differs from what is committed; run 'just gen' and commit the result" >&2
        git status --short -- "${paths[@]}" >&2
        exit 1
    fi
    echo "check-contract: up to date"

# Verify every design decision is cited by a plan task, the roadmap or Not built.
check-trace:
    bun tools/trace/trace.ts

# Install the pinned Ansible toolchain and collections into target/infra.
[private]
infra-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    venv=target/infra
    stamp="$venv/.installed"
    if [ ! -f "$stamp" ] || [ infra/requirements.txt -nt "$stamp" ] || [ infra/requirements.yml -nt "$stamp" ]; then
        python3 -m venv "$venv"
        "$venv/bin/pip" install --quiet -r infra/requirements.txt
        ANSIBLE_CONFIG="{{ justfile_directory() }}/infra/ansible.cfg" "$venv/bin/ansible-galaxy" collection install --upgrade -r infra/requirements.yml -p "$venv/collections"
        touch "$stamp"
    fi

# Lint the Ansible role and playbooks, check their syntax, and dry-run the rendered Quadlet units.
check-infra: infra-tools
    @infra/check.sh

# Run decision traceability in strict mode (roadmap-only citations fail).
check-trace-strict:
    bun tools/trace/trace.ts --strict

# Run web unit tests.
test-web:
    bun run --filter @pranata/web test

# Run API tests (needs `just db-up`).
test-api:
    cargo test --workspace --locked

# Recovery: create an Administrator when the first run stopped before it did; prints the Access Link once.
create-admin host username name: infra-tools
    PRANATA_ADMIN_USERNAME={{ quote(username) }} PRANATA_ADMIN_NAME={{ quote(name) }} \
    PATH="{{ justfile_directory() }}/target/infra/bin:$PATH" ANSIBLE_CONFIG="{{ justfile_directory() }}/infra/ansible.cfg" \
        ansible-playbook -i "${PRANATA_INVENTORY:?set PRANATA_INVENTORY to your inventory file}" infra/create-admin.yml -l {{ host }}

# Provision a disposable Debian 13 + systemd container with the role and check the Installation.
infra-test: infra-tools images
    infra/test/run.sh

# Provision one Installation (or `all`) from the inventory named by $PRANATA_INVENTORY.
provision host: infra-tools
    PATH="{{ justfile_directory() }}/target/infra/bin:$PATH" ANSIBLE_CONFIG="{{ justfile_directory() }}/infra/ansible.cfg" \
        ansible-playbook -i "${PRANATA_INVENTORY:?set PRANATA_INVENTORY to your inventory file}" infra/provision.yml -l {{ host }}

# Deploy a version to one Installation (or `all`); a failed health check rolls it back.
deploy host version: infra-tools
    PATH="{{ justfile_directory() }}/target/infra/bin:$PATH" ANSIBLE_CONFIG="{{ justfile_directory() }}/infra/ansible.cfg" \
        ansible-playbook -i "${PRANATA_INVENTORY:?set PRANATA_INVENTORY to your inventory file}" infra/deploy.yml -l {{ host }} -e pranata_version={{ version }}

# Fail when real village data leaks outside allowed paths.
check-village-data:
    tools/no-village-data.sh

# Build the api and web images for linux/amd64 (podman). PRANATA_SKIP_IMAGES=1 reuses
# loaded ones; PRANATA_IMAGE_SOURCE=<repo url> labels them so ghcr links them to the repo.
images tag="local":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "${PRANATA_SKIP_IMAGES:-}" = "1" ]; then
        echo "images: PRANATA_SKIP_IMAGES=1, reusing the images already in podman"
        exit 0
    fi
    labels=()
    if [ -n "${PRANATA_IMAGE_SOURCE:-}" ]; then
        labels=(--label "org.opencontainers.image.source=$PRANATA_IMAGE_SOURCE")
    fi
    podman build --platform linux/amd64 "${labels[@]}" -f apps/api/Containerfile -t pranata-api:{{ tag }} .
    podman build --platform linux/amd64 "${labels[@]}" -f apps/web/Containerfile -t pranata-web:{{ tag }} .

# Scan the api and web images for HIGH and CRITICAL vulnerabilities that have a
# fix available (D-48), with Trivy from its official image. Fails on any finding.
# The images are exported to target/trivy and the vulnerability database is
# cached in target/trivy/cache. An exception needs a per-CVE entry with a
# justification in .trivyignore, and only when no fix exists.
scan-images tag="local":
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    dir="$root/target/trivy"
    mkdir -p "$dir/cache" "$dir/images"
    status=0
    for name in api web; do
        rm -f "$dir/images/$name.tar"
        podman save --format docker-archive -o "$dir/images/$name.tar" "localhost/pranata-$name:{{ tag }}"
        echo "scan-images: pranata-$name:{{ tag }}"
        podman run --rm \
            -v "$dir/cache:/root/.cache:z" \
            -v "$dir/images:/images:ro,z" \
            -v "$root/.trivyignore:/work/.trivyignore:ro,z" \
            -w /work \
            docker.io/aquasec/trivy:0.76.2 image --input "/images/$name.tar" \
            --severity HIGH,CRITICAL --ignore-unfixed --exit-code 1 --no-progress \
            --ignorefile /work/.trivyignore || status=1
    done
    exit "$status"

# Run end-to-end tests against the production images in a resource-limited pod.
e2e: images
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    trap '"$root/infra/dev/stack.sh" down --no-init; "$root/infra/dev/stack.sh" down' EXIT
    "$root/infra/dev/stack.sh" up
    cd "$root/apps/web"
    bunx playwright install chromium
    bunx playwright test

# Run Lighthouse CI (mobile, `/`) and the bundle size limit against the images.
perf: images
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    trap '"$root/infra/dev/stack.sh" down' EXIT
    "$root/infra/dev/stack.sh" up
    "$root/load/seed.sh"
    rm -rf "$root/target/lighthouse"
    cd "$root/apps/web"
    bunx playwright install chromium
    bun run build
    bun run size
    bunx lhci autorun --config=lighthouserc.cjs
    for report in "$root"/target/lighthouse/*.report.json; do
        jq -r '"lighthouse: performance \(.categories.performance.score) accessibility \(.categories.accessibility.score) lcp \(.audits["largest-contentful-paint"].displayValue) tbt \(.audits["total-blocking-time"].displayValue)"' "$report"
    done

# Run the k6 load test (300 visitors) against the images, with k6 from its official image.
load: images
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    trap '"$root/infra/dev/stack.sh" down' EXIT
    "$root/infra/dev/stack.sh" up
    "$root/load/seed.sh"
    mkdir -p "$root/target/load"
    podman run --rm --network=host --user 0:0 \
        -v "{{ justfile_directory() }}/load:/load:ro,z" \
        -v "{{ justfile_directory() }}/target/load:/out:z" \
        -e BASE_URL="https://localhost:${PRANATA_E2E_PORT:-8443}" \
        docker.io/grafana/k6:2.4.0 run --summary-export=/out/summary.json /load/public.js

# Load seed data (plan R1-E).
seed:
    @echo "seed: not implemented yet (plan R1-E)"
