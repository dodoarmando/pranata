#!/usr/bin/env bash
# Gives the e2e stack the content the load test needs: a password for the
# e2e Admin (redeeming the Access Link), a hero photo and a short overview.
# Usage: load/seed.sh   (after infra/dev/stack.sh up)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
origin="https://localhost:${PRANATA_E2E_PORT:-8443}"
link="$(cat "$root/target/e2e/access-link.txt")"
token="${link##*#}"
photo="$root/apps/web/e2e/fixtures/gps-photo.jpg"
jar="$(mktemp)"
trap 'rm -f "$jar"' EXIT
curl_api() { curl -fsS -k -b "$jar" -c "$jar" "$@"; }

curl_api -X POST "$origin/api/auth/access-links/redeem" \
    -H 'content-type: application/json' \
    -d "{\"token\":\"$token\",\"password\":\"e2e-password-123\"}" >/dev/null
csrf="$(curl_api -X POST "$origin/api/auth/login" -H 'content-type: application/json' \
    -d '{"username":"e2e","password":"e2e-password-123"}' | jq -r .csrf_token)"

asset="$(curl_api -X POST "$origin/api/admin/media" -H "x-csrf-token: $csrf" \
    -F kind=image -F "file=@$photo;type=image/jpeg" | jq -r .id)"
for _ in $(seq 1 60); do
    status="$(curl_api "$origin/api/admin/media/$asset" | jq -r .status)"
    [ "$status" = ready ] && break
    sleep 1
done
[ "$status" = ready ] || { echo "seed: photo not ready" >&2; exit 1; }

updated="$(curl_api "$origin/api/admin/village/profile" | jq -r .updated_at)"
body="$(jq -n --arg asset "$asset" --arg at "$updated" '{
    hero_asset_id: $asset, updated_at: $at,
    overview: {type: "doc", content: [{type: "paragraph",
        content: [{type: "text", text: "Ringkasan desa untuk uji beban."}]}]}}')"
curl_api -X PUT "$origin/api/admin/village/profile" -H "x-csrf-token: $csrf" \
    -H 'content-type: application/json' -d "$body" >/dev/null
echo "seed: hero photo and overview set"
