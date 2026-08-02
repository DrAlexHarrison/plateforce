#!/usr/bin/env bash
# Builds the browser bundle from this checkout and asks it what it can do.
#
# The row builds what it asks, which is what scripts/capability-surfaces.txt requires of
# every row. Measured on a checkout of 8c0f85e holding a bundle built five commits earlier:
# asking the bundle alone reported `3 of 3 surfaces reported and matched`, exit 0, having
# compared the terminal and the R package at that commit against a browser from another one.
#
# The build writes to the other stream, because the caller reads this one as the surface's
# answer.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"$root/scripts/build-web.sh" release >&2
exec node "$root/scripts/capability-from-wasm.mjs"
