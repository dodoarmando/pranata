# Dev-only defaults for running the API on this machine, sourced by the
# `just dev` recipe. The
# passwords match infra/dev/postgres-init.sql. A variable already set wins.
root="$(git rev-parse --show-toplevel)"
export PRANATA_DATABASE_URL="${PRANATA_DATABASE_URL:-postgres://pranata_app:pranata_app_dev@127.0.0.1:55432/pranata}"
export PRANATA_MIGRATION_DATABASE_URL="${PRANATA_MIGRATION_DATABASE_URL:-postgres://pranata_owner:pranata_owner_dev@127.0.0.1:55432/pranata}"
export PRANATA_PUBLIC_ORIGIN="${PRANATA_PUBLIC_ORIGIN:-http://localhost:3000}"
# `localhost`, not 127.0.0.1: the web dev server may listen on ::1 only.
export PRANATA_WEB_INTERNAL_URL="${PRANATA_WEB_INTERNAL_URL:-http://localhost:3000}"
export PRANATA_PURGE_TOKEN="${PRANATA_PURGE_TOKEN:-dev-purge-token}"
export PRANATA_MEDIA_DIR="${PRANATA_MEDIA_DIR:-$root/target/dev-media}"
export PRANATA_LISTEN="${PRANATA_LISTEN:-127.0.0.1:8080}"
unset root
