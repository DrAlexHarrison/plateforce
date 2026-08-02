#!/bin/sh
# Two facts about the engine sources the package carries, and it fails on either.
#
# Without them an R surface can ship an engine older than the browser's, and every number
# would still agree with itself. That is the drift this product exists to publish about,
# reproduced in our own release process.
#
# One: the copy is byte for byte what `tools/sync-engine.sh` wrote, so nothing has been
# edited in place. Two: what it wrote is the repository's current engine.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")
record="$package_root/src/rust/ENGINE-SOURCE"

repository=${1:-}
if [ -z "$repository" ]; then
    repository=$(CDPATH= cd -- "$package_root/../.." && pwd)
fi

if [ ! -f "$record" ]; then
    echo "no record of where the engine copy came from: run tools/sync-engine.sh" >&2
    exit 1
fi

recorded_revision=$(sed -n 's/^revision //p' "$record")
recorded_digest=$(sed -n 's/^digest //p' "$record")
carried_digest=$(sh "$here/engine-digest.sh" "$package_root")

printf 'copied from      %s\n' "$recorded_revision"
printf 'recorded digest  %s\n' "$recorded_digest"
printf 'digest now       %s\n' "$carried_digest"

status=0
if [ "$recorded_digest" != "$carried_digest" ]; then
    echo "the engine copy has been edited in place: run tools/sync-engine.sh" >&2
    status=1
fi

if [ "$recorded_revision" != worktree ]; then
    head_revision=$(git -C "$repository" rev-parse --short HEAD)
    printf 'repository at    %s\n' "$head_revision"
    if [ "$recorded_revision" != "$head_revision" ]; then
        echo "the engine copy is not this repository's engine: run tools/sync-engine.sh" >&2
        status=1
    fi
fi

exit "$status"
