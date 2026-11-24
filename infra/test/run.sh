#!/usr/bin/env bash
# Provisions a disposable Debian 13 + systemd container with infra/provision.yml
# (tls internal, no firewall, no ssh hardening), then asserts the Installation
# works and that a failing deploy rolls back. Everything it creates is named
# pranata-infra-test*; it touches nothing else.
#
# Needs the images localhost/pranata-{api,web}:$PRANATA_INFRA_TEST_TAG (default local; `just images [tag]`).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export PATH="$root/target/infra/bin:$PATH"
export ANSIBLE_CONFIG="$root/infra/ansible.cfg"
cd "$root"

name="pranata-infra-test"
s3_name="$name-s3"
s3_image="docker.io/dxflrs/garage:v2.5.0@sha256:23b16d34e0348d9aca9b960a7429a7e6d494974b5288bfb8f6c3ab856895f71e"
image="localhost/$name:latest"
label="pranata.infra-test.owner=run.sh"
work="$root/target/infra-test"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' apps/api/Cargo.toml | head -n 1)"
tag="${PRANATA_INFRA_TEST_TAG:-local}"
tagged=()

cleanup() {
    local status=$?
    if [ $status -ne 0 ]; then
        echo "--- journal of the test container (last 25 lines)" >&2
        podman exec "$name" journalctl --no-pager -n 25 -o cat >&2 2>/dev/null || true
        podman exec "$name" su -s /bin/sh pranata -c 'XDG_RUNTIME_DIR=/run/user/$(id -u) journalctl --user --no-pager -n 40 -o cat' >&2 2>/dev/null || true
    fi
    # The S3 server shares the test container's network, so it goes first.
    if [ -n "$(podman ps -a --quiet --filter "name=^$s3_name\$" --filter "label=$label")" ]; then
        podman rm -f -v "$s3_name" >/dev/null 2>&1 || true
    fi
    if [ -n "$(podman ps -a --quiet --filter "name=^$name\$" --filter "label=$label")" ]; then
        podman rm -f -v "$name" >/dev/null 2>&1 || true
    fi
    for ref in "${tagged[@]}"; do
        podman untag "$ref" "$ref" >/dev/null 2>&1 || true
    done
}
trap cleanup EXIT

if [ -n "$(podman ps -a --quiet --filter "name=^$name\$")" ] &&
    [ -z "$(podman ps -a --quiet --filter "name=^$name\$" --filter "label=$label")" ]; then
    echo "infra-test: a container named $name exists without the label $label; it is not ours, remove it yourself" >&2
    exit 3
fi
if [ -n "$(podman ps -a --quiet --filter "name=^$s3_name\$")" ] &&
    [ -z "$(podman ps -a --quiet --filter "name=^$s3_name\$" --filter "label=$label")" ]; then
    echo "infra-test: a container named $s3_name exists without the label $label; it is not ours, remove it yourself" >&2
    exit 3
fi
podman rm -f -v "$s3_name" >/dev/null 2>&1 || true
podman rm -f -v "$name" >/dev/null 2>&1 || true

mkdir -p "$work"
podman build --quiet -f infra/test/Containerfile -t "$image" infra/test >/dev/null

# The units pull pranata-{api,web}:<version>; give the local builds that tag
# for the duration of the test and ship them to the container as an archive.
for app in api web; do
    ref="localhost/pranata-$app:$version"
    if ! podman image exists "$ref"; then
        podman tag "localhost/pranata-$app:$tag" "$ref"
        tagged+=("$ref")
    fi
done
rm -f "$work/images.tar"
podman save --multi-image-archive -o "$work/images.tar" \
    "localhost/pranata-api:$version" "localhost/pranata-web:$version"

podman run -d --name "$name" --label "$label" --privileged --systemd=always "$image" >/dev/null
for _ in $(seq 1 60); do
    state="$(podman exec "$name" systemctl is-system-running 2>/dev/null || true)"
    case "$state" in running | degraded) break ;; esac
    sleep 1
done
echo "infra-test: systemd in the container is $state"

# An S3-compatible server for the backups: Garage, in the test container's
# network namespace so the repository is s3:http://127.0.0.1:3900/... (D-51).
s3_key_id="GK$(openssl rand -hex 12)"
s3_secret="$(openssl rand -hex 32)"
restic_password="infra-test restic/password+1"
cat >"$work/garage.toml" <<TOML
metadata_dir = "/tmp/garage/meta"
data_dir = "/tmp/garage/data"
db_engine = "sqlite"
replication_factor = 1
rpc_bind_addr = "127.0.0.1:3901"
rpc_secret = "$(openssl rand -hex 32)"

[s3_api]
s3_region = "garage"
api_bind_addr = "127.0.0.1:3900"
TOML
podman run -d --name "$s3_name" --label "$label" --network "container:$name" --tmpfs /tmp/garage \
    -v "$work/garage.toml:/etc/garage.toml:ro,z" "$s3_image" >/dev/null
garage() { podman exec "$s3_name" /garage "$@"; }
for _ in $(seq 1 30); do
    garage status >/dev/null 2>&1 && break
    sleep 1
done
node_id="$(garage node id -q | cut -d@ -f1)"
garage layout assign -z dc1 -c 1G "$node_id" >/dev/null
garage layout apply --version 1 >/dev/null
garage bucket create pranata-backups >/dev/null
garage key import --yes -n infra-test "$s3_key_id" "$s3_secret" >/dev/null
garage bucket allow --read --write --owner pranata-backups --key "$s3_key_id" >/dev/null
echo "infra-test: garage $(podman exec "$s3_name" /garage --version | head -n 1) serves pranata-backups"

# Stand-in for Healthchecks.io: logs the path of every ping it gets.
podman cp infra/test/ping-server.py "$name:/tmp/ping-server.py"
podman exec -d "$name" python3 /tmp/ping-server.py /tmp/pings.log

cat >"$work/inventory.yml" <<YAML
all:
  hosts:
    $name:
      ansible_connection: containers.podman.podman
      ansible_python_interpreter: /usr/bin/python3
      pranata_domain: localhost
      pranata_version: "$version"
      pranata_tls: internal
      pranata_image_registry: localhost
      # The nested user and its subordinate ids must all fit in the 65537 ids of a rootless container.
      pranata_user_uid: 65536
      pranata_pull_images: false
      pranata_images_archive: "$work/images.tar"
      pranata_village_code: "14.05.03.2002"
      pranata_admin_username: infra-test
      pranata_admin_name: Infra Test
      pranata_db_owner_password: "test owner/password+1"
      pranata_db_app_password: "test app/password+1"
      pranata_purge_token: infra-test-purge-token
      pranata_restic_password: "$restic_password"
      pranata_restic_repository: s3:http://127.0.0.1:3900/pranata-backups
      pranata_restic_env:
        AWS_ACCESS_KEY_ID: "$s3_key_id"
        AWS_SECRET_ACCESS_KEY: "$s3_secret"
        AWS_DEFAULT_REGION: garage
      pranata_monitor_ping_urls:
        backup: http://127.0.0.1:8099/backup
        restore: http://127.0.0.1:8099/restore
        disk: http://127.0.0.1:8099/disk
YAML

play() { ansible-playbook -i "$work/inventory.yml" "$@"; }
in_container() { podman exec "$name" "$@"; }

echo "infra-test: provision"
play infra/provision.yml --skip-tags firewall,ssh | tee "$work/provision.log"
if ! grep -q 'Access Link for infra-test' "$work/provision.log"; then
    echo "infra-test: the first run did not print the Access Link" >&2
    exit 1
fi

echo "infra-test: provision again (nothing to bootstrap)"
play infra/provision.yml --skip-tags firewall,ssh | tee "$work/provision-2.log"
if grep -q 'Access Link for' "$work/provision-2.log"; then
    echo "infra-test: the second run created another Administrator" >&2
    exit 1
fi

grep -A1 'TASK \[pranata : Initialise the restic repository\]' "$work/provision-2.log" | grep -q 'skipping' ||
    { echo "infra-test: the second run initialised the restic repository again" >&2; exit 1; }

echo "infra-test: assertions"
health="$(in_container curl -fsk https://localhost/api/healthz)"
echo "  healthz: $health"
case "$health" in *"\"version\":\"$version\""*) ;; *)
    echo "infra-test: healthz does not report $version" >&2
    exit 1
    ;;
esac

ready="$(in_container curl -fsk https://localhost/api/readyz)"
echo "  readyz: $ready"
case "$ready" in *'"status":"ok"'*) ;; *)
    echo "infra-test: readyz is not ok" >&2
    exit 1
    ;;
esac

# D-36: Caddy sends the security headers on web pages and the API sends them on
# /api; each must arrive exactly once.
for path in / /api/healthz; do
    headers="$(in_container curl -sk -o /dev/null -D - "https://localhost$path")"
    for expected in 'strict-transport-security: max-age=31536000' 'x-content-type-options: nosniff' 'referrer-policy: strict-origin-when-cross-origin'; do
        count="$(echo "$headers" | tr -d '\r' | grep -ci "^$expected$" || true)"
        [ "$count" = "1" ] ||
            { echo "infra-test: $path has $count copies of '$expected', want 1" >&2; echo "$headers" >&2; exit 1; }
    done
done
echo "  security headers once on / and /api/healthz"

purge="$(in_container curl -sk -o /dev/null -w '%{http_code}' -X POST https://localhost/_internal/cache/purge)"
echo "  /_internal/cache/purge: $purge"
[ "$purge" = "404" ] || { echo "infra-test: the purge hook is reachable through Caddy" >&2; exit 1; }

in_container sh -c 'echo probe > /home/pranata/media/public/probe.txt'
headers="$(in_container curl -skI https://localhost/media/probe.txt)"
echo "$headers" | sed 's/^/  /'
echo "$headers" | grep -qi '^cache-control: public, max-age=31536000, immutable' ||
    { echo "infra-test: /media/ lacks the immutable Cache-Control" >&2; exit 1; }

echo "infra-test: backups, restore check and pings"
# Runs a shell snippet as the Pranata user with its systemd user manager's bus.
as_pranata() { in_container su -s /bin/sh pranata -c "export XDG_RUNTIME_DIR=/run/user/65536; $1"; }
# Runs a command in a transient user unit that loads the env files the way the
# real units do (EnvironmentFile=), so the test never sources them with a shell.
with_env() {
    as_pranata "systemd-run --user --quiet --pipe --wait --collect \
        -p EnvironmentFile=/home/pranata/.config/pranata/backup.env \
        -p EnvironmentFile=/home/pranata/.config/pranata/monitor.env -- $1"
}
restic_as_pranata() { with_env "restic $1"; }
# The SQL goes in on stdin, so it needs no quoting through su and podman exec.
psql_live() { printf '%s\n' "$2" | podman exec -i "$name" su -s /bin/sh pranata -c "XDG_RUNTIME_DIR=/run/user/65536 podman exec -i pranata-postgres psql -U postgres -d $1 -tA"; }
scratch_exists() { psql_live postgres "SELECT count(*) FROM pg_database WHERE datname = 'pranata_restore_check'"; }
pings() { in_container cat /tmp/pings.log 2>/dev/null || true; }

pinned_restic="$(sed -n 's/^pranata_restic_version: "\(.*\)"$/\1/p' infra/roles/pranata/defaults/main.yml)"
[ -n "$pinned_restic" ] || { echo "infra-test: no pinned restic version in the role defaults" >&2; exit 1; }
case "$(in_container restic version)" in "restic $pinned_restic "*) echo "  restic $pinned_restic is installed" ;; *)
    echo "infra-test: restic does not report the pinned version $pinned_restic" >&2
    exit 1
    ;;
esac
for timer in pranata-backup pranata-restore-check pranata-disk-check; do
    as_pranata "systemctl --user is-enabled $timer.timer" >/dev/null ||
        { echo "infra-test: $timer.timer is not enabled" >&2; exit 1; }
    as_pranata "systemctl --user show -p Persistent --value $timer.timer" | grep -qx yes ||
        { echo "infra-test: $timer.timer is not persistent" >&2; exit 1; }
done
[ "$(in_container stat -c %a:%U /home/pranata/.config/pranata/backup.env)" = "600:pranata" ] ||
    { echo "infra-test: backup.env is not 0600 pranata" >&2; exit 1; }
if in_container grep -rqF -e "$restic_password" -e "$s3_secret" /home/pranata/.config/systemd /usr/local/libexec/pranata; then
    echo "infra-test: a restic secret leaked into a unit or script" >&2
    exit 1
fi

# A private original, owned by the api's uid, which only the user namespace can read.
as_pranata 'podman unshare sh -c "mkdir -p /home/pranata/media/originals/ab /home/pranata/media/public/ab && echo original > /home/pranata/media/originals/ab/abcdef.jpg && echo variant > /home/pranata/media/public/ab/abcdef-480.webp && chown -R 65532:65532 /home/pranata/media/originals"'
psql_live pranata "UPDATE village SET overview = '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"before-backup\"}]}]}'" >/dev/null

as_pranata 'systemctl --user start pranata-backup.service'
snapshots="$(restic_as_pranata 'snapshots --json')"
echo "$snapshots" | python3 -I -c '
import json, sys
snapshots = json.load(sys.stdin)
assert len(snapshots) == 1, f"expected one snapshot, got {len(snapshots)}"
assert "pranata" in snapshots[0]["tags"], snapshots[0]["tags"]
' || { echo "infra-test: restic snapshots does not show one snapshot tagged pranata" >&2; exit 1; }
echo "  one snapshot tagged pranata"
# Capture the listing first: grep -q closing the pipe early would trip pipefail.
listing="$(restic_as_pranata 'ls latest')"
case "$listing" in *originals/ab/abcdef.jpg*) ;; *)
    echo "infra-test: the snapshot lacks the media originals" >&2
    exit 1
    ;;
esac
# A restore without the variants would leave every image broken.
case "$listing" in *public/ab/abcdef-480.webp*) ;; *)
    echo "infra-test: the snapshot lacks the public variants" >&2
    exit 1
    ;;
esac
pings | grep -qx 'GET /backup' || { echo "infra-test: the backup did not ping" >&2; exit 1; }

# Change the live row, then restore the backup into the scratch database by hand.
psql_live pranata "UPDATE village SET overview = '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"after-backup\"}]}]}'" >/dev/null
psql_live postgres 'CREATE DATABASE pranata_restore_check' >/dev/null
restic_as_pranata 'dump latest /home/pranata/.local/state/pranata/backup/pranata.dump' |
    podman exec -i "$name" su -s /bin/sh pranata -c 'XDG_RUNTIME_DIR=/run/user/65536 podman exec -i pranata-postgres pg_restore -U postgres -d pranata_restore_check --no-owner --no-privileges --exit-on-error'
restored="$(psql_live pranata_restore_check 'SELECT overview FROM village')"
case "$restored" in *before-backup*) echo "  the restored row has its old value: $restored" ;; *)
    echo "infra-test: the scratch database lacks the old row (got: $restored)" >&2
    exit 1
    ;;
esac
psql_live postgres 'DROP DATABASE pranata_restore_check' >/dev/null

# The real restore check cleans up after itself and leaves the live database alone.
as_pranata 'systemctl --user start pranata-restore-check.service'
[ "$(scratch_exists)" = "0" ] || { echo "infra-test: the scratch database survived the restore check" >&2; exit 1; }
live="$(psql_live pranata 'SELECT overview FROM village')"
case "$live" in *after-backup*) ;; *)
    echo "infra-test: the live database changed during the restore check (got: $live)" >&2
    exit 1
    ;;
esac
# Leave the Village as provisioned for the steps that follow.
psql_live pranata 'UPDATE village SET overview = NULL' >/dev/null
pings | grep -qx 'GET /restore' || { echo "infra-test: the restore check did not ping" >&2; exit 1; }

# A failing step still drops the scratch database and pings /fail.
if with_env 'env RESTIC_PASSWORD=wrong /usr/local/libexec/pranata/restore-check.sh'; then
    echo "infra-test: the restore check should fail with a wrong restic password" >&2
    exit 1
fi
[ "$(scratch_exists)" = "0" ] || { echo "infra-test: a failed restore check left the scratch database" >&2; exit 1; }
pings | grep -qx 'GET /restore/fail' || { echo "infra-test: the failed restore check did not ping /fail" >&2; exit 1; }
if with_env 'env RESTIC_PASSWORD=wrong /usr/local/libexec/pranata/backup.sh'; then
    echo "infra-test: the backup should fail with a wrong restic password" >&2
    exit 1
fi
pings | grep -qx 'GET /backup/fail' || { echo "infra-test: the failed backup did not ping /fail" >&2; exit 1; }

# The disk check pings, and pings /fail above its limit.
as_pranata 'systemctl --user start pranata-disk-check.service' || true
pings | grep -Eqx 'GET /disk(/fail)?' || { echo "infra-test: the disk check did not ping" >&2; exit 1; }
if with_env 'env PRANATA_DISK_LIMIT=-1 /usr/local/libexec/pranata/disk-check.sh'; then
    echo "infra-test: the disk check should fail above its limit" >&2
    exit 1
fi
pings | grep -qx 'GET /disk/fail' || { echo "infra-test: the failed disk check did not ping /fail" >&2; exit 1; }
[ "$(restic_as_pranata 'snapshots --json' | python3 -I -c 'import json, sys; print(len(json.load(sys.stdin)))')" = "1" ] ||
    { echo "infra-test: a failing run created a snapshot" >&2; exit 1; }

echo "infra-test: a failing deploy rolls back"
if play infra/deploy.yml -e pranata_version=9.9.9 | tee "$work/deploy.log"; then
    echo "infra-test: deploying a missing version should have failed" >&2
    exit 1
fi
grep -q "rolled back to $version" "$work/deploy.log" ||
    { echo "infra-test: the deploy did not report the rollback" >&2; exit 1; }
health="$(in_container curl -fsk https://localhost/api/healthz)"
case "$health" in *"\"version\":\"$version\""*) ;; *)
    echo "infra-test: after the rollback healthz does not report $version" >&2
    exit 1
    ;;
esac
echo "infra-test: the api health command reports healthy"
in_container su -s /bin/sh pranata -c 'XDG_RUNTIME_DIR=/run/user/65536 podman healthcheck run pranata-api'

echo "infra-test: provision keeps the running release when the inventory names another"
play infra/provision.yml --skip-tags firewall,ssh -e pranata_version=9.9.9 | tee "$work/provision-3.log"
grep -q "WARNING: $name runs $version but the" "$work/provision-3.log" ||
    { echo "infra-test: no warning about the differing inventory version" >&2; exit 1; }
health="$(in_container curl -fsk https://localhost/api/healthz)"
case "$health" in *"\"version\":\"$version\""*) ;; *)
    echo "infra-test: provision moved the release away from $version" >&2
    exit 1
    ;;
esac

echo "infra-test: the ssh step refuses a root connection"
if play infra/provision.yml --tags ssh | tee "$work/ssh.log"; then
    echo "infra-test: the ssh step should refuse to lock out root" >&2
    exit 1
fi
grep -q "Ansible connects as root" "$work/ssh.log" ||
    { echo "infra-test: the ssh refusal message is missing" >&2; exit 1; }

echo "infra-test: create-admin recovery"
if PRANATA_ADMIN_USERNAME='Bad Name' PRANATA_ADMIN_NAME='Nobody' play infra/create-admin.yml; then
    echo "infra-test: an invalid username should be refused" >&2
    exit 1
fi
PRANATA_ADMIN_USERNAME=second.admin PRANATA_ADMIN_NAME='Second Admin' play infra/create-admin.yml | tee "$work/create-admin.log"
grep -q 'Access Link for second.admin' "$work/create-admin.log" ||
    { echo "infra-test: create-admin printed no Access Link" >&2; exit 1; }
echo "infra-test: ok"
