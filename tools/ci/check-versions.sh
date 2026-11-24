#!/usr/bin/env bash
# Fails when a file that release-please bumps disagrees with the manifest, so the
# binary (`--version`, /api/healthz), the web app and the image tag cannot drift.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

expected="$(jq -r '."."' .release-please-manifest.json)"
status=0

check() {
	local file="$1" found="$2"
	if [ "$found" != "$expected" ]; then
		echo "check-versions: $file is '$found', the manifest says '$expected'" >&2
		status=1
	fi
}

check apps/api/Cargo.toml "$(sed -n '/^\[package\]/,/^\[/{s/^version = "\(.*\)"$/\1/p}' apps/api/Cargo.toml)"
check Cargo.lock "$(awk '/^name = "pranata"$/ {getline; gsub(/version = |"/, ""); print; exit}' Cargo.lock)"
check package.json "$(jq -r '.version // "missing"' package.json)"
check apps/web/package.json "$(jq -r '.version // "missing"' apps/web/package.json)"

if [ "$status" -eq 0 ]; then
	echo "check-versions: all files at $expected"
fi
exit "$status"
