#!/usr/bin/env bash
# Report which workstream changed which registry entry, and fail on a crossing.
#
#   entry-ownership.sh --workstream WS-E2 [--base <ref>]
#   entry-ownership.sh --audit
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$here/entry-ownership.py" "$@"
