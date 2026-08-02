#!/usr/bin/env bash
#
# Asks of a built DMG the three questions Gatekeeper asks, because each fails differently
# and only the third decides what a reader with no internet on first launch sees.
#
#   ./scripts/verify-macos-artefact.sh <dmg>
#
#   codesign  the signature is intact
#   spctl     Gatekeeper accepts the policy the signature was made under
#   stapler   the notarisation ticket travelled inside the file rather than being fetched

set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <dmg>" >&2
  exit 2
fi

image="$1"
if [ ! -f "$image" ]; then
  echo "$image: no such file" >&2
  exit 1
fi

mounted="$(mktemp -d)"
hdiutil attach "$image" -nobrowse -readonly -mountpoint "$mounted" >/dev/null
trap 'hdiutil detach "$mounted" -quiet >/dev/null 2>&1 || true; rmdir "$mounted" 2>/dev/null || true' EXIT

application="$(find "$mounted" -maxdepth 1 -name '*.app' | head -1)"
if [ -z "$application" ]; then
  echo "$image carries no application" >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "$application"
spctl -a -vvv -t install "$application"
xcrun stapler validate "$application"

echo "codesign ok, spctl accepted, staple valid"
