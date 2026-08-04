#!/usr/bin/env bash
# The browser's answer to the one committed request, from a bundle built out of this checkout.
#
# The bundle is built here rather than read as found: `scripts/result-from-browser.mjs` alone
# imports `web/pkg/`, whatever built it and whenever. Measured on this checkout: with the
# browser's own source edited to pin a revision no caller pinned, and `web/pkg/` left as it
# was, this gate reported four of four surfaces computing the committed result; rebuilding the
# bundle and asking again named the field and the value. A gate that passes on a stale artifact
# certifies a binary that may predate the change under review, which is what
# `scripts/result-parity-surfaces.txt` says at the top no row may do.
#
# The Python row builds what it asks in scripts/install-python-wheel.sh and the R row in
# scripts/result-from-r.sh, each for the same reason.
#
# The build writes to the other stream, because the caller reads this one as the surface's
# answer.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bash "$root/scripts/build-web.sh" >&2

bundle="$root/web/pkg/plateforce_wasm.js"
if [ ! -f "$bundle" ]; then
    echo "the bundle was reported built and $bundle is not there" >&2
    exit 1
fi

cd "$root"
exec node scripts/result-from-browser.mjs
