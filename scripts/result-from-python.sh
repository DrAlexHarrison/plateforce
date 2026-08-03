#!/usr/bin/env bash
# The notebook's answer to the one committed request.
#
# The wheel is built from this checkout and installed into an environment of its own before
# it is asked, because a row that read whichever `plateforce` happened to be importable would
# answer for whatever was last installed on the machine. scripts/install-python-wheel.sh
# states that in full and is shared with the capability row.
#
# The build writes to the other stream, because the caller reads this one as the surface's
# answer.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
python="$("$root/scripts/install-python-wheel.sh")"
exec "$python" "$root/scripts/result-from-python.py"
