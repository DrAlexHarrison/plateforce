#!/bin/sh
# Copies the registry into this crate so the source distribution carries it.
#
# `build.rs` reads the registry at compile time and embeds it, because a wheel installed
# from an index has no clone of this repository beside it to read one from. maturin packages
# the workspace root and the sibling crates into the sdist, which is why `plateforce-registry`
# compiles there, but the registry is a directory rather than a crate and nothing carried it.
# Every wheel job builds from the sdist with no checkout, by design, so all five platforms
# died at once in `build.rs` with "reading ../../registry: No such file or directory".
#
# It has to live inside this directory rather than be reached from it: maturin's `include`
# resolves against the crate root and refuses a path containing `..`. The R package meets the
# same wall for the same reason and answers it the same way, in bindings/r/tools/sync-engine.sh.
#
# One commit is copied rather than the working tree, so the registry and the validator that
# reads it are coherent by construction even while this tree is being edited by several
# agents at once. PLATEFORCE_SYNC_FROM=worktree takes the working tree instead.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")

repository=${1:-}
if [ -z "$repository" ]; then
    repository=$(CDPATH= cd -- "$package_root/../.." && pwd)
fi

if [ ! -d "$repository/registry" ]; then
    echo "no registry at $repository/registry" >&2
    echo "pass the repository root as the first argument" >&2
    exit 1
fi

destination="$package_root/registry"

source_kind=worktree
revision=
if [ "${PLATEFORCE_SYNC_FROM:-commit}" = "commit" ] && \
   git -C "$repository" rev-parse --verify HEAD >/dev/null 2>&1; then
    source_kind=commit
    revision=$(git -C "$repository" rev-parse --short HEAD)
fi

# Removed rather than overwritten, so a rule deleted from the registry cannot survive in the
# copy and ship inside a wheel after nobody can find it in the tree.
rm -rf "$destination"
mkdir -p "$destination"

if [ "$source_kind" = commit ]; then
    git -C "$repository" archive HEAD registry | tar -C "$destination" --strip-components=1 -xf -
else
    tar -C "$repository/registry" -cf - . | tar -C "$destination" -xf -
fi

# `git archive` stamps every member with the commit's time, so a copy taken from a newer
# commit can carry an older mtime than the last build, and cargo skips the rebuild. The
# embedded registry then reports as current while being stale, which is the drift this
# product exists to publish about, reproduced in our own build.
find "$destination" -type f -exec touch {} +

rules=$(find "$destination" -name '*.toml' | wc -l | tr -d ' ')
files=$(find "$destination" -type f | wc -l | tr -d ' ')
if [ "$rules" -eq 0 ]; then
    echo "copied no registry rule file from $repository/registry" >&2
    exit 1
fi

# Both counts, because they differ: the walk that reads rules filters on the toml extension
# and the registry also carries the VERSION file that names its revision. One number here
# would disagree with the sdist gate's and read as a discrepancy.
if [ "$source_kind" = commit ]; then
    echo "registry copied from commit $revision: $files files, $rules of them rules"
else
    echo "registry copied from the working tree at $repository: $files files, $rules of them rules"
fi
