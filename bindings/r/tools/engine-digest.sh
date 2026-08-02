#!/bin/sh
# The digest of the engine sources and the registry the package carries. One definition,
# used by the script that writes the record and by the script that checks it.
set -eu

package_root=$1
cd "$package_root"

listing=$(
    {
        for crate in plateforce-registry plateforce-core plateforce-analysis; do
            find "src/rust/crates/$crate" -type f -not -path '*/target/*'
        done
        find inst/registry -type f
    } | LC_ALL=C sort
)

count=$(printf '%s\n' "$listing" | grep -c .)
if [ "$count" -lt 20 ]; then
    echo "only $count files to digest: the engine copy is not in place" >&2
    exit 1
fi

printf '%s\n' "$listing" | xargs sha256sum | sha256sum | cut -d' ' -f1
