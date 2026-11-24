#!/usr/bin/env bash
# Runs the production images (api, web), Postgres and Caddy in one Podman pod
# with the VPS budget (2 CPUs, 2 GiB), for the end-to-end, Lighthouse and k6
# gates. Everything it creates is named pranata-e2e*; it touches nothing else.
#
#   infra/dev/stack.sh up [--no-init]    start (--no-init: no Village, no Admin)
#   infra/dev/stack.sh down [--no-init]  remove the pod and its media directory
#
# Env: PRANATA_IMAGE_TAG (default local), PRANATA_E2E_PORT (default 8443;
# 8444 with --no-init).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cmd="${1:-}"
variant="main"
[ "${2:-}" = "--no-init" ] && variant="noinit"

tag="${PRANATA_IMAGE_TAG:-local}"
if [ "$variant" = "noinit" ]; then
    pod="pranata-e2e-noinit"
    port="${PRANATA_E2E_NOINIT_PORT:-8444}"
else
    pod="pranata-e2e"
    port="${PRANATA_E2E_PORT:-8443}"
fi
origin="https://localhost:$port"
out_dir="$root/target/e2e"
media_dir="$out_dir/media-$variant"
postgres_image="docker.io/library/postgres:19"
caddy_image="docker.io/library/caddy:2.11.9"
village_code="14.05.03.2002"
purge_token="e2e-purge-token"
db="postgres://%s:%s@127.0.0.1:5432/pranata"
# shellcheck disable=SC2059
owner_url="$(printf "$db" pranata_owner pranata_owner_dev)"
# shellcheck disable=SC2059
app_url="$(printf "$db" pranata_app pranata_app_dev)"

api_env=(
    -e "PRANATA_DATABASE_URL=$app_url"
    -e "PRANATA_MIGRATION_DATABASE_URL=$owner_url"
    -e "PRANATA_PUBLIC_ORIGIN=$origin"
    -e "PRANATA_WEB_INTERNAL_URL=http://127.0.0.1:3000"
    -e "PRANATA_PURGE_TOKEN=$purge_token"
    -e "PRANATA_MEDIA_DIR=/media"
    -e "PRANATA_LISTEN=0.0.0.0:8080"
)

down() {
    podman pod rm -f "$pod" >/dev/null 2>&1 || true
    # Files written by the api belong to a subordinate uid; remove them from
    # inside the user namespace.
    podman unshare rm -rf "$media_dir" >/dev/null 2>&1 || true
    echo "stack: $pod removed"
}

# One-shot CLI run inside the pod (shares the database on localhost).
api_cli() {
    podman run --rm --pod "$pod" "${api_env[@]}" "localhost/pranata-api:$tag" "$@"
}

wait_for() {
    local what="$1" timeout="$2"
    shift 2
    for _ in $(seq 1 "$timeout"); do
        if "$@" >/dev/null 2>&1; then return 0; fi
        sleep 1
    done
    echo "stack: timed out waiting for $what" >&2
    return 1
}

up() {
    down >/dev/null
    mkdir -p "$out_dir"
    rm -f "$out_dir/access-link.txt"

    podman pod create --name "$pod" --cpus=2 --memory=2g \
        --add-host api:127.0.0.1 --add-host web:127.0.0.1 \
        -p "127.0.0.1:$port:443" >/dev/null

    # The api runs as uid 65532 and must be able to write the media directory.
    # (A bind mount: podman re-chowns named volumes to each container's user.)
    mkdir -p "$media_dir"
    podman unshare chown 65532:65532 "$media_dir"

    podman run -d --pod "$pod" --name "$pod-db" \
        -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=pranata \
        -v "$root/infra/dev/postgres-init.sql:/docker-entrypoint-initdb.d/00-init.sql:ro,z" \
        "$postgres_image" >/dev/null
    # The init scripts run on a temporary server first, so wait until the real
    # one answers over TCP and the roles exist.
    wait_for postgres 60 podman exec "$pod-db" \
        psql -h 127.0.0.1 -U pranata_owner -d pranata -tAc 'select 1'

    api_cli migrate >/dev/null
    if [ "$variant" = "main" ]; then
        api_cli village init --code "$village_code"
        api_cli user create-admin e2e --name "Admin E2E" >"$out_dir/access-link.txt"
    fi

    podman run -d --pod "$pod" --name "$pod-api" "${api_env[@]}" \
        -v "$media_dir:/media:z" "localhost/pranata-api:$tag" serve >/dev/null
    podman run -d --pod "$pod" --name "$pod-web" \
        -e "PRANATA_API_INTERNAL_URL=http://127.0.0.1:8080" \
        -e "PRANATA_PURGE_TOKEN=$purge_token" \
        "localhost/pranata-web:$tag" >/dev/null
    podman run -d --pod "$pod" --name "$pod-caddy" \
        -v "$root/infra/dev/Caddyfile:/etc/caddy/Caddyfile:ro,z" \
        -v "$media_dir:/srv/media:ro,z" \
        "$caddy_image" >/dev/null

    wait_for "api health" 90 curl -fsk "$origin/api/healthz"
    if [ "$variant" = "main" ]; then
        wait_for "the public page" 60 curl -fsk "$origin/"
    else
        wait_for "the web server" 60 curl -sk -o /dev/null "$origin/"
    fi
    echo "stack: $pod ready at $origin"
}

case "$cmd" in
    up)
        trap 'status=$?; if [ $status -ne 0 ]; then
            for c in db api web caddy; do echo "--- $pod-$c" >&2; podman logs --tail 30 "$pod-$c" >&2 || true; done
            down; fi' EXIT
        up
        ;;
    down) down ;;
    *)
        echo "usage: $0 up|down [--no-init]" >&2
        exit 2
        ;;
esac
