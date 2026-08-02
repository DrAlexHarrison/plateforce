#!/usr/bin/env bash
#
# Asserts that the Windows artefacts are what the release claims: present, not truncated,
# and carrying the browser build this tree produced.
#
#   ./scripts/verify-windows-artefact.sh <directory holding the artefacts>
#
# The installer and the package both embed the interface inside the executable rather than
# beside it, so the payload is read by asking the extracted program what it carries.

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <directory>" >&2
  exit 2
fi
artefacts="$1"

# A truncated upload leaves a file that exists and does not run. The band is wide because
# the interface and the module inside dominate the size, and it is narrow enough that half
# an upload fails it.
readonly SMALLEST_PLAUSIBLE_BYTES=$((3 * 1024 * 1024))
readonly LARGEST_PLAUSIBLE_BYTES=$((60 * 1024 * 1024))

in_the_tree="$(sha256sum web/pkg/plateforce_wasm_bg.wasm | cut -c1-64)"
status=0

check_size() {
  local file="$1" size
  size="$(stat -c %s "$file" 2>/dev/null || stat -f %z "$file")"
  if [ "$size" -lt "$SMALLEST_PLAUSIBLE_BYTES" ] || [ "$size" -gt "$LARGEST_PLAUSIBLE_BYTES" ]; then
    echo "$(basename "$file") is ${size} bytes, outside the plausible band" >&2
    return 1
  fi
}

check_payload() {
  local label="$1" executable="$2"
  local reported
  reported="$("$executable" --capability)"
  if [ "${reported#sha256:}" != "$in_the_tree" ]; then
    echo "${label} payload carries a browser build this tree did not produce" >&2
    echo "  web/pkg: sha256 ${in_the_tree}" >&2
    echo "  payload: ${reported}" >&2
    return 1
  fi
  echo "${label} payload wasm matches"
}

installer="$(ls "${artefacts}"/*-setup.exe 2>/dev/null | head -1 || true)"
if [ -z "$installer" ]; then
  echo "no -setup.exe in ${artefacts}" >&2
  status=1
else
  check_size "$installer" || status=1
  scratch="$(mktemp -d)"
  7z x -y -o"$scratch" "$installer" >/dev/null
  program="$(find "$scratch" -name 'plateforce-desktop.exe' | head -1)"
  if [ -z "$program" ]; then
    echo "the installer carries no plateforce-desktop.exe" >&2
    status=1
  else
    check_payload "nsis" "$program" || status=1
  fi
  rm -rf "$scratch"
fi

package="$(ls "${artefacts}"/*.msix 2>/dev/null | head -1 || true)"
if [ -z "$package" ]; then
  echo "no .msix in ${artefacts}, so the Store route has nothing to submit" >&2
  status=1
else
  check_size "$package" || status=1
  scratch="$(mktemp -d)"
  makeappx.exe unpack /p "$package" /d "$scratch" /o >/dev/null
  grep -q runFullTrust "${scratch}/AppxManifest.xml" \
    || { echo "the package declares no runFullTrust, which a packaged desktop application needs" >&2; status=1; }
  check_payload "msix" "${scratch}/plateforce-desktop.exe" || status=1
  rm -rf "$scratch"
fi

exit "$status"
