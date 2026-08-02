#!/usr/bin/env bash
#
# Refuses a Linux binary that needs a newer glibc than the oldest distribution plateforce
# claims to run on. The floor is invisible until a user reports that the application will
# not start, and the message they get names a symbol version rather than anything they can
# act on, so it is checked here instead.
#
#   ./scripts/verify-glibc-floor.sh <binary> [<binary>...]
#
# 2.35 is Ubuntu 22.04, which is also the oldest base carrying libwebkit2gtk-4.1-dev, so
# nothing older can build the shell in the first place. Debian 12 is 2.36 and RHEL 9 is
# 2.34; RHEL is reached by the static binary rather than by any desktop artefact.

set -euo pipefail

readonly OLDEST_GLIBC_PLATEFORCE_RUNS_ON="2.35"

if [ "$#" -eq 0 ]; then
  echo "usage: $0 <binary> [<binary>...]" >&2
  exit 2
fi

as_number() { echo "$1" | awk -F. '{ printf "%d%03d\n", $1, $2 }'; }

status=0
for binary in "$@"; do
  if [ ! -f "$binary" ]; then
    echo "$binary: no such file" >&2
    status=1
    continue
  fi

  # Every versioned glibc symbol the binary imports. The highest is what the loader on the
  # target machine has to satisfy.
  highest="$(objdump -T "$binary" 2>/dev/null \
    | grep -oE 'GLIBC_[0-9]+\.[0-9]+' \
    | sed 's/GLIBC_//' \
    | sort -V \
    | tail -1)"

  if [ -z "$highest" ]; then
    echo "$binary: no GLIBC symbol, floor ${OLDEST_GLIBC_PLATEFORCE_RUNS_ON}, ok"
    continue
  fi

  if [ "$(as_number "$highest")" -gt "$(as_number "$OLDEST_GLIBC_PLATEFORCE_RUNS_ON")" ]; then
    echo "$binary: highest GLIBC symbol ${highest}, floor ${OLDEST_GLIBC_PLATEFORCE_RUNS_ON}, too new" >&2
    echo "  built against a newer glibc than the oldest distribution this ships to" >&2
    status=1
    continue
  fi

  echo "$binary: highest GLIBC symbol ${highest}, floor ${OLDEST_GLIBC_PLATEFORCE_RUNS_ON}, ok"
done

exit "$status"
