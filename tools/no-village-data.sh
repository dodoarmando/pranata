#!/usr/bin/env bash
# Fails when real village names appear in tracked files outside the allowed paths.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

pattern='Sidomukti|Sukamaju'
allowed='^(apps/api/seed/|docs/|CONTEXT\.md:)|(^|/)fixtures/|^tools/no-village-data\.sh:'

# grep exits 1 when nothing matches (fine) and >=2 on a real error (fatal).
# The allow-list also exempts this script, which contains the pattern itself.
raw=$(git ls-files -z | xargs -0 grep -IinE "$pattern" -- || { rc=$?; [ "$rc" -le 1 ] || exit "$rc"; })
hits=$(printf '%s\n' "$raw" | grep -vE "$allowed" || true)

if [ -n "$hits" ]; then
	echo "Village-specific names found outside allowed paths:" >&2
	echo "$hits" >&2
	exit 1
fi
echo "No village-specific data found outside allowed paths."
