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
destination="$package_root/src/rust/crates"
registry="$package_root/inst/registry"

repository=${1:-}
if [ -z "$repository" ]; then
    repository=$(CDPATH= cd -- "$package_root/../.." && pwd)
fi

for crate in plateforce-registry plateforce-core plateforce-analysis; do
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

rm -rf "$destination" "$registry"
mkdir -p "$destination" "$registry"

take() {
    from=$1
    into=$2
    if [ "$source_kind" = commit ]; then
        git -C "$repository" archive HEAD "$from" | tar -C "$into" --strip-components=1 -xf -
    else
        tar -C "$repository/$from" --exclude=target -cf - . | tar -C "$into" -xf -
    fi
}

for crate in plateforce-registry plateforce-core plateforce-analysis; do
    mkdir -p "$destination/$crate"
    if [ "$source_kind" = commit ]; then
        git -C "$repository" archive HEAD "crates/$crate" \
            | tar -C "$destination/$crate" --strip-components=2 -xf -
    else
        tar -C "$repository/crates/$crate" --exclude=target -cf - . \
            | tar -C "$destination/$crate" -xf -
    fi
done

take registry "$registry"

floor=$(grep -v '^ *#' "$here/msrv" | grep -v '^ *$' | head -1 | tr -d ' \r')
python3 "$here/resolve-manifests.py" "$repository" "$destination" "$floor"

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
