#!/usr/bin/env bash
# Fails when real village names appear in tracked files outside the allowed paths.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

pattern='Sidomukti|Sukamaju'
allowed='^(apps/api/seed/|docs/|CONTEXT\.md:)|(^|/)fixtures/|^tools/no-village-data\.sh:'

hits=$(git ls-files -z | xargs -0 grep -IinE "$pattern" -- 2>/dev/null | grep -vE "$allowed" || true)

if [ -n "$hits" ]; then
	echo "Village-specific names found outside allowed paths:" >&2
	echo "$hits" >&2
	exit 1
fi
echo "No village-specific data found outside allowed paths."
