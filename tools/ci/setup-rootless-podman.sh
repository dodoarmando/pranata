#!/usr/bin/env bash
# Prepares a GitHub-hosted Ubuntu 24.04 runner for rootless Podman pods with
# enforced cgroup v2 cpu and memory limits (infra/dev/stack.sh fails otherwise).
#
# A rootless Podman can only set pod limits through a user systemd manager that
# has been delegated the cpu and memory controllers. So this script:
#   1. delegates the controllers to every user@.service,
#   2. starts (lingers) the runner's user manager,
#   3. exports the user bus so later steps talk to it and podman uses the
#      systemd cgroup manager.
# Run it as the runner user; it uses sudo for the system parts.
set -euo pipefail

uid="$(id -u)"
user="$(id -un)"

if [ "$(stat -fc %T /sys/fs/cgroup)" != "cgroup2fs" ]; then
    echo "setup-rootless-podman: cgroup v2 is required" >&2
    exit 1
fi

# Ubuntu 24.04 can block unprivileged user namespaces through AppArmor.
sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0 >/dev/null 2>&1 || true

sudo apt-get update -qq
sudo apt-get install -y -qq --no-install-recommends podman uidmap slirp4netns passt dbus-user-session >/dev/null

sudo mkdir -p /etc/systemd/system/user@.service.d
printf '[Service]\nDelegate=cpu cpuset io memory pids\n' |
    sudo tee /etc/systemd/system/user@.service.d/delegate.conf >/dev/null
sudo systemctl daemon-reload

sudo loginctl enable-linger "$user"
sudo systemctl restart "user@$uid.service"

runtime_dir="/run/user/$uid"
for _ in $(seq 1 30); do
    [ -S "$runtime_dir/bus" ] && break
    sleep 1
done
if [ ! -S "$runtime_dir/bus" ]; then
    echo "setup-rootless-podman: the user bus did not appear at $runtime_dir/bus" >&2
    exit 1
fi

export XDG_RUNTIME_DIR="$runtime_dir"
export DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_dir/bus"
if [ -n "${GITHUB_ENV:-}" ]; then
    {
        echo "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR"
        echo "DBUS_SESSION_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS"
    } >>"$GITHUB_ENV"
fi

controllers="/sys/fs/cgroup/user.slice/user-$uid.slice/user@$uid.service/cgroup.controllers"
echo "setup-rootless-podman: delegated controllers: $(cat "$controllers")"
for c in cpu memory; do
    if ! grep -qw "$c" "$controllers"; then
        echo "setup-rootless-podman: the $c controller is not delegated to the user manager" >&2
        exit 1
    fi
done
podman --version
