#!/bin/sh
# Puts the engine copy in step with the repository before anything reads it.
#
# The copy is a build artifact and is not tracked, so a checkout that has one has whatever
# the last run left. Taking it because it exists is how a build compiles an engine older
# than the repository's while every number it produces agrees with itself, which is the
# drift this product exists to publish about.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")

if [ ! -d "$package_root/src/rust/crates/plateforce-core" ]; then
    sh "$here/sync-engine.sh" >/dev/null
    exit 0
fi

if sh "$here/check-engine-version.sh" >/dev/null 2>&1; then
    exit 0
fi

echo "the engine copy is not this repository's engine, taking a current one" >&2
sh "$here/sync-engine.sh" >/dev/null
