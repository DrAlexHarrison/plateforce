#!/usr/bin/env bash
#
# Submits one archive to Apple's notary service and waits for the answer, retrying while the
# service itself is unavailable.
#
#   ./scripts/notarise.sh <archive> [attempts]
#
# The App Store Connect credentials are read from the environment, because they are secrets
# and an argument list is visible to every process on the machine:
#
#   APPLE_API_KEY_PATH   the .p8 private key on disk
#   APPLE_API_KEY        the key id
#   APPLE_API_ISSUER     the issuer id
#
# Both macOS artefacts submit through here, so the retry is written once and the disk image
# and the command line program cannot drift apart in how long they wait.
#
# notarytool's output is redirected to a file rather than piped to tee, because a pipeline
# reports the exit status of its LAST command. Piped, the loop below reads tee's status,
# which is success whatever the notary service said, so it would take the first answer as
# final and never retry. The log is printed afterwards instead.

set -euo pipefail

archive="${1:?name the archive to submit}"
attempts="${2:-5}"

for required in APPLE_API_KEY_PATH APPLE_API_KEY APPLE_API_ISSUER; do
  if [ -z "${!required:-}" ]; then
    echo "$0: $required is not set, and the submission cannot authenticate without it" >&2
    exit 2
  fi
done

if [ ! -f "$archive" ]; then
  echo "$0: $archive: no such file" >&2
  exit 1
fi

log="$(mktemp)"
trap 'rm -f "$log"' EXIT INT TERM

submit() {
  xcrun notarytool submit "$archive" \
    --key "$APPLE_API_KEY_PATH" \
    --key-id "$APPLE_API_KEY" \
    --issuer "$APPLE_API_ISSUER" \
    --wait --timeout 30m > "$log" 2>&1
}

attempt=1
until submit; do
  cat "$log"
  if [ "$attempt" -ge "$attempts" ]; then
    echo "notarisation of $(basename "$archive") did not complete in $attempt attempts" >&2
    # The submission id, so the same submission can be asked about by hand rather than
    # started again. `notarytool log <id>` says what the service objected to.
    grep -oE 'id: [0-9a-f-]+' "$log" | head -1 >&2 || true
    echo "the signed artefact is uploaded by the run and can be notarised by hand against" >&2
    echo "this same binary rather than against a rebuilt one" >&2
    exit 1
  fi
  echo "notarisation attempt $attempt did not complete, waiting"
  sleep $((attempt * 60))
  attempt=$((attempt + 1))
done

cat "$log"
