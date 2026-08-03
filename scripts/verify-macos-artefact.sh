#!/usr/bin/env bash
#
# Asks of a built DMG the three questions Gatekeeper asks, because each fails differently
# and only the third decides what a reader with no internet on first launch sees.
#
#   ./scripts/verify-macos-artefact.sh <dmg> [--mounts-only]
#
#   codesign  the signature is intact
#   spctl     Gatekeeper accepts the policy the signature was made under
#   stapler   the notarisation ticket travelled inside the file rather than being fetched
#
# The three questions need a certificate and the mounting before them does not, so
# `--mounts-only` runs the half an unsigned build can answer. Without it the first signed
# release would also be the first time anything opened one of these images.

set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "usage: $0 <dmg> [--mounts-only]" >&2
  exit 2
fi

mounts_only="no"
if [ "$#" -eq 2 ]; then
  if [ "$2" != "--mounts-only" ]; then
    echo "$0: $2 is not an option this takes" >&2
    exit 2
  fi
  mounts_only="yes"
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

if [ "$mounts_only" = "yes" ]; then
  echo "the image mounts and carries $(basename "$application")"
  exit 0
fi

codesign --verify --deep --strict --verbose=2 "$application"
spctl -a -vvv -t install "$application"

# The ticket is on the image, so the image is what carries it. The workflow staples the
# DMG, a mounted DMG is read only, and stapling a container cannot reach inside it, so
# asking the application for a ticket asks the one file that was never given one. The
# reader downloads the image, and the image is what has to answer offline on first launch.
xcrun stapler validate "$image"

echo "codesign ok, spctl accepted, staple valid on the image"
