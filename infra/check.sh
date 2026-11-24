#!/usr/bin/env bash
# Static checks for the infra files: ansible-lint, playbook syntax, the Caddyfile
# routes against the dev Caddyfile, and a Quadlet dry-run of the rendered units.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$root/target/infra/bin:$PATH"
export ANSIBLE_CONFIG="$root/infra/ansible.cfg"
cd "$root"

echo "check-infra: ansible-lint"
ansible-lint --offline -c infra/.ansible-lint infra/provision.yml infra/deploy.yml infra/roles

echo "check-infra: syntax"
for playbook in provision deploy; do
    ansible-playbook --syntax-check -i localhost, "infra/$playbook.yml"
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
vars='{"pranata_domain":"localhost","pranata_version":"0.0.0","pranata_release":"0.0.0","pranata_tls":"internal","pranata_db_owner_password":"owner","pranata_db_app_password":"app","pranata_purge_token":"token","pranata_media_dir":"/home/pranata/media"}'
render() {
    ansible localhost -c local -i localhost, -m ansible.builtin.template \
        -a "src=$root/infra/roles/pranata/templates/$1 dest=$2" \
        -e "@$root/infra/roles/pranata/defaults/main.yml" -e "$vars" >"$work/render.log" || { cat "$work/render.log" >&2; exit 1; }
}

echo "check-infra: Caddyfile routes match infra/dev/Caddyfile"
render Caddyfile.j2 "$work/Caddyfile"
# Compare from the `encode` line: the global options and site address differ on purpose.
if ! diff <(sed -n '/encode zstd gzip/,$p' infra/dev/Caddyfile) <(sed -n '/encode zstd gzip/,$p' "$work/Caddyfile"); then
    echo "check-infra: Caddyfile.j2 routes differ from infra/dev/Caddyfile" >&2
    exit 1
fi

echo "check-infra: Quadlet dry-run"
mkdir -p "$work/units"
for unit in pranata.network pranata-postgres.container pranata-api.container pranata-web.container pranata-caddy.container; do
    render "$unit" "$work/units/$unit"
done
# Quadlet differs between Podman versions, so the dry-run uses the Podman of
# Debian 13, the production target, not whatever the runner has installed.
podman build --quiet --target quadlet -f infra/test/Containerfile -t localhost/pranata-infra-quadlet:latest infra/test >/dev/null
podman run --rm --volume "$work/units:/units:ro,z" localhost/pranata-infra-quadlet:latest sh -c '
    quadlet=$(ls /usr/lib/podman/quadlet /usr/libexec/podman/quadlet 2>/dev/null | head -n 1)
    podman --version >&2
    QUADLET_UNIT_DIRS=/units "$quadlet" -dryrun -user' >"$work/dryrun.txt"
cat "$work/dryrun.txt"
for service in pranata-postgres pranata-api pranata-web pranata-caddy pranata-network; do
    grep -q "^---$service.service---" "$work/dryrun.txt" || { echo "check-infra: Quadlet generated no $service.service" >&2; exit 1; }
done
echo "check-infra: ok"
