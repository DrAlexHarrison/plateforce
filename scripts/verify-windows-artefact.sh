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

# A truncated upload leaves a file that exists and does not run, and the size is all that
# separates it from a whole one. The floor was 3 MiB against an installer NSIS actually
# produces at 2,380,954 bytes, measured on run 30738883059: NSIS compresses a payload whose
# deb and rpm figures were taken uncompressed, so the band rejected every correct build.
# 1.5 MiB is under that measurement and above half of it, so half a file still fails.
readonly SMALLEST_PLAUSIBLE_BYTES=$((3 * 1024 * 1024 / 2))
readonly LARGEST_PLAUSIBLE_BYTES=$((120 * 1024 * 1024))

in_the_tree="$(sha256sum web/pkg/plateforce_wasm_bg.wasm | cut -c1-64)"
status=0
checked=0
absent=""

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
  absent="$absent the installer"
else
  checked=$((checked + 1))
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
  absent="$absent the Store package"
else
  checked=$((checked + 1))
  check_size "$package" || status=1
  scratch="$(mktemp -d)"
  makeappx.exe unpack /p "$package" /d "$scratch" /o >/dev/null
  grep -q runFullTrust "${scratch}/AppxManifest.xml" \
    || { echo "the package declares no runFullTrust, which a packaged desktop application needs" >&2; status=1; }
  check_payload "msix" "${scratch}/plateforce-desktop.exe" || status=1
  rm -rf "$scratch"
fi

# The count with its denominator, so a run that inspected one artefact cannot read as a run
# that inspected both. Zero is a failure: a check that examined nothing is not a pass.
echo "checked ${checked} of 2 Windows artefacts${absent:+, absent:${absent}}"
if [ "$checked" -eq 0 ]; then
  echo "nothing in ${artefacts} was a Windows artefact, so nothing was verified" >&2
  status=1
fi

exit "$status"
