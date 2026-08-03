#!/bin/sh
# Copies the engine crates and the registry into the R package so the built tarball is
# self-contained.
#
# The three crates are not on crates.io and `cargo vendor` does not vendor a path
# dependency reached through `[patch.crates-io]`, measured 2026-08-01: the vendor
# directory came back holding every third-party crate and none of the plateforce ones.
# So an installable source package has to carry the engine sources itself. Git carries
# one copy, under `crates/`; this copy is a build artifact and is not tracked.
#
# One commit is copied rather than the working tree, and the revision is printed. A
# registry read at one moment and a validator read at another can disagree, and this tree
# is edited by several people at once: copying a commit makes the pair coherent by
# construction. PLATEFORCE_SYNC_FROM=worktree takes the working tree instead.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")
destination="$package_root/src/rust"
registry="$package_root/inst/registry"

# Each engine crate and the directory it takes inside the package, which is deliberately not
# the name of the crate. A tar member is only required to hold 100 bytes. The package
# directory and the engine's own path spend 76 of them on the longest rule, leaving 24 for
# everything this script puts in front of it, and `crates/` plus the `plateforce-` that every
# crate shares with the package it is already sitting inside spend 18 on words the path has
# said once already. Taking those two levels out buys 18 bytes for every file in the engine
# at once, where shortening one rule's name buys them for one file. The package names inside
# the manifests are untouched, so cargo still resolves `plateforce-analysis`; it is the
# folder that is shorter. `tools/resolve-manifests.py` is handed this same mapping, so the
# path dependencies between the three copies point at where they actually landed.
engine='plateforce-registry:registry plateforce-core:core plateforce-analysis:analysis'

# The layout above replaced `src/rust/crates/<crate>/`, and a checkout that ran the older
# script still holds that tree. It is no longer written, no longer ignored, and still walked
# by tools/check-portable-paths.sh, so on every machine that predates the rename the gate
# reports the old long paths and reads as though the fix did not work. Removed here rather
# than left to each reader to discover, because the copy is this script's to own.
rm -rf "$destination/crates"

repository=${1:-}
if [ -z "$repository" ]; then
    repository=$(CDPATH= cd -- "$package_root/../.." && pwd)
fi

for pair in $engine; do
    crate=${pair%:*}
    if [ ! -f "$repository/crates/$crate/Cargo.toml" ]; then
        echo "no engine source at $repository/crates/$crate" >&2
        echo "pass the repository root as the first argument" >&2
        exit 1
    fi
done
if [ ! -d "$repository/registry" ]; then
    echo "no registry at $repository/registry" >&2
    exit 1
fi

source_kind=worktree
revision=
if [ "${PLATEFORCE_SYNC_FROM:-commit}" = "commit" ] && \
   git -C "$repository" rev-parse --verify HEAD >/dev/null 2>&1; then
    source_kind=commit
    revision=$(git -C "$repository" rev-parse --short HEAD)
fi

rm -rf "$registry"
mkdir -p "$registry"

take() {
    from=$1
    into=$2
    if [ "$source_kind" = commit ]; then
        git -C "$repository" archive HEAD "$from" | tar -C "$into" --strip-components=1 -xf -
    else
        tar -C "$repository/$from" --exclude=target -cf - . | tar -C "$into" -xf -
    fi
}

# Each copy is cleared on its own rather than by emptying the directory above it, which is
# now `src/rust` and holds the binding crate's own tracked sources as well.
for pair in $engine; do
    crate=${pair%:*}
    into="$destination/${pair#*:}"
    rm -rf "$into"
    mkdir -p "$into"
    if [ "$source_kind" = commit ]; then
        git -C "$repository" archive HEAD "crates/$crate" \
            | tar -C "$into" --strip-components=2 -xf -
    else
        tar -C "$repository/crates/$crate" --exclude=target -cf - . \
            | tar -C "$into" -xf -
    fi
    # The engine's own suite runs in the repository, against the repository's fixtures, and
    # no R check runs it. Carrying it costs the one thing a source tarball cannot spend:
    # `R CMD check` reads six of these names as non-portable file paths, because a tar member
    # is only required to hold 100 bytes and this project names a test after the sentence it
    # proves. Removed after the copy rather than filtered during it, because the two tar
    # dialects on the three check platforms spell an exclusion differently.
    rm -rf "$into/tests"
done

take registry "$registry"

# `git archive` stamps every member with the commit's time, so a copy taken from a newer
# commit can carry an older mtime than the last build and cargo will skip the rebuild. The
# digest guard compares content and reports the copy as current while the linked engine is
# stale, which is the drift this product exists to publish about, in our own build.
for pair in $engine; do
    find "$destination/${pair#*:}" -type f -exec touch {} +
done
find "$registry" -type f -exec touch {} +

floor=$(grep -v '^ *#' "$here/msrv" | grep -v '^ *$' | head -1 | tr -d ' \r')
python3 "$here/resolve-manifests.py" "$repository" "$destination" "$floor" $engine

{
    printf 'revision %s\n' "${revision:-worktree}"
    printf 'digest %s\n' "$(sh "$here/engine-digest.sh" "$package_root")"
} > "$package_root/src/rust/ENGINE-SOURCE"

if [ "$source_kind" = commit ]; then
    echo "engine and registry copied from commit $revision"
else
    echo "engine and registry copied from the working tree at $repository"
fi
echo "registry: $(find "$registry" -type f | wc -l) files"
