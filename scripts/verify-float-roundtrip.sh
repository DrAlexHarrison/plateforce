#!/usr/bin/env bash
#
# Asserts that every workspace shipping code resolves serde_json with `float_roundtrip`.
#
#   ./scripts/verify-float-roundtrip.sh
#
# serde_json's default parser is not correctly rounded, so a double written by one surface
# and read by another comes back a unit in the last place different. Nothing is wrong
# physically; what it destroys is two surfaces reporting the same number, which is the
# property this product sells.
#
# The check reads the resolved feature graph rather than running the tests, because the tests
# pass either way: a manifest edit removing the feature is in a file no test covers, so the
# suite is exactly what such an edit preserves. It asks every workspace rather than the root
# one, because the two detached workspaces are where the root's declaration cannot reach, and
# both of them ship.

set -uo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root" || exit 2

readonly REQUIRED_FEATURE="float_roundtrip"

# Discovered rather than listed, so a workspace added later cannot join without meeting this.
mapfile -t workspaces < <(
  git ls-files '*Cargo.toml' \
    | xargs -I{} dirname {} \
    | while read -r directory; do
        cargo metadata --format-version 1 --no-deps --manifest-path "$directory/Cargo.toml" \
          2>/dev/null | python3 -c "import json,sys; print(json.load(sys.stdin)['workspace_root'])" \
          2>/dev/null
      done \
    | sort -u
)

if [ "${#workspaces[@]}" -eq 0 ]; then
  echo "no workspace found to check" >&2
  exit 2
fi

status=0
for workspace in "${workspaces[@]}"; do
  named="${workspace#"$repository_root"/}"
  [ "$named" = "$workspace" ] && named="."

  graph="$(cd "$workspace" && cargo tree -e features -i serde_json 2>/dev/null)"

  if [ -z "$graph" ]; then
    echo "  $named: serde_json is not in this graph, so nothing to carry the feature"
    continue
  fi

  if printf '%s' "$graph" | grep -q "feature \"${REQUIRED_FEATURE}\""; then
    echo "  $named: serde_json resolves with ${REQUIRED_FEATURE}"
  else
    echo "  $named: serde_json resolves WITHOUT ${REQUIRED_FEATURE}" >&2
    echo "      a double written here and read on another surface comes back different" >&2
    status=1
  fi
done

exit "$status"
