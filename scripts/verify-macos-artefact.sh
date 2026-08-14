#!/usr/bin/env bash
#
# Asks of a built DMG the three questions Gatekeeper asks, because each fails differently
# and only the third decides what a reader with no internet on first launch sees.
#
#   ./scripts/verify-macos-artefact.sh <dmg> [--mounts-only]
#   ./scripts/verify-macos-artefact.sh <executable> --executable
#
#   codesign  the signature is intact
#   spctl     Gatekeeper accepts the policy the signature was made under
#   stapler   the notarisation ticket travelled inside the file rather than being fetched
#
# The three questions need a certificate and the mounting before them does not, so
# `--mounts-only` runs the half an unsigned build can answer. Without it the first signed
# release would also be the first time anything opened one of these images.
#
# `--executable` asks the same of the command line program, which is a loose Mach-O file
# rather than an image. It answers the first two questions and not the third: `stapler`
# attaches a ticket to UDIF disk images, code-signed bundles and flat installer packages,
# and a bare executable is none of those. Its ticket stays where the notary service
# published it, and spctl below is the question that reads it.
#
# The assessment type is `open` rather than `exec`, because `exec` assesses an application
# and answers `rejected (the code is valid but does not seem to be an app)` for a Mach-O
# file whatever its ticket says. Measured against a notarised binary and an un-notarised
# one signed by the same identity: under `exec` both are rejected, and under `open` with
# the primary signature the notarised one is accepted as `Notarized Developer ID` while
# the other is rejected as `Unnotarized Developer ID`, which is the property being asked
# about.

set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "usage: $0 <dmg> [--mounts-only] | $0 <executable> --executable" >&2
  exit 2
fi

mounts_only="no"
executable="no"
if [ "$#" -eq 2 ]; then
  case "$2" in
    --mounts-only) mounts_only="yes" ;;
    --executable) executable="yes" ;;
    *)
      echo "$0: $2 is not an option this takes" >&2
      exit 2
      ;;
  esac
fi

image="$1"
if [ ! -f "$image" ]; then
  echo "$image: no such file" >&2
  exit 1
fi

if [ "$executable" = "yes" ]; then
  codesign --verify --strict --verbose=2 "$image"

  # Gatekeeper reads this file's ticket from Apple rather than from inside it, so the answer
  # depends on a lookup over the network and a ticket published moments ago can take a little
  # while to be visible. Asked once, a release would fail on the gap between the notary
  # service accepting the submission and the answer reaching the edge that serves it.
  #
  # spctl's own exit status decides whether to ask again, so it is run on its own line rather
  # than through a pipe: a pipeline reports the status of its last command, which would be
  # the reader of the output rather than the question being asked.
  assessment="$(mktemp)"
  trap 'rm -f "$assessment"' EXIT INT TERM
  attempt=1
  while true; do
    accepted=yes
    spctl -a -t open --context context:primary-signature -vv "$image" > "$assessment" 2>&1 || accepted=no
    cat "$assessment"
    # An un-notarised signature is a settled answer rather than a slow one, and asking six
    # times cannot change it. Only an absent ticket is worth waiting for.
    if [ "$accepted" = yes ] || ! grep -q "Unnotarized Developer ID" "$assessment"; then
      break
    fi
    if [ "$attempt" -ge 6 ]; then
      echo "Gatekeeper did not accept $image in $attempt attempts" >&2
      exit 1
    fi
    echo "the ticket is not visible yet, asking again"
    sleep $((attempt * 15))
    attempt=$((attempt + 1))
  done

  # spctl accepts more than one kind of signature, so the exit status alone would pass a file
  # that carries no ticket at all. The source is what separates them.
  if [ "$accepted" != yes ] || ! grep -q "source=Notarized Developer ID" "$assessment"; then
    echo "$image is not accepted as notarised software" >&2
    exit 1
  fi

  echo "codesign ok, spctl accepts it as notarised software"
  exit 0
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
