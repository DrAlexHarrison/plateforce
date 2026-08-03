#!/usr/bin/env bash
# Builds a wheel from this checkout, installs it, and asks it what it can do.
#
# The row builds what it asks, which is what scripts/capability-surfaces.txt requires of every
# row, and scripts/install-python-wheel.sh states in full why a row that read whichever
# `plateforce` happened to be importable would answer for a different tree.
#
# The manifest is read off the running wheel's own exports, so this asks the extension that
# was just compiled rather than the source beside it.
#
# The build writes to the other stream, because the caller reads this one as the surface's
# answer.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
python="$("$root/scripts/install-python-wheel.sh")"
exec "$python" -c 'import plateforce; print(plateforce.capability_json())'
