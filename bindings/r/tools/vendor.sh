#!/bin/sh
# Bundles every third-party crate into src/rust/vendor.tar.xz, so the install compiles
# with the network off and against exactly the tree that was checked.
#
# Reproducibly: a tar whose member order and timestamps vary produces a different
# checksum on every build, and the checksum is what tells a reviewer the bundle did not
# change. Every file this writes is untracked, so running it twice leaves the same tree.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")
crate="$package_root/src/rust"

if [ ! -d "$crate/crates/plateforce-core" ]; then
    sh "$here/sync-engine.sh" >/dev/null
fi

# The mtime every member is stamped with. Taken from the package rather than from the
# clock, so two builds of one version produce one bundle.
stamp=$(sed -n 's/^Version: *//p' "$package_root/DESCRIPTION" | head -1)
mtime="2026-01-01 00:00:00Z"

cd "$crate"
rm -rf vendor vendor.tar.xz
cargo vendor --versioned-dirs vendor > /dev/null

# The directory is written as a placeholder rather than as a path, because cargo resolves
# a relative one against the config file it reads and the install puts that file
# somewhere neither this script nor cargo can predict.
cat > vendor-config.toml <<'CONFIG'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "@VENDOR_DIR@"
CONFIG

tar --sort=name --mtime="$mtime" --owner=0 --group=0 --numeric-owner \
    --format=gnu -cf - vendor | xz -T0 -9 > vendor.tar.xz
rm -rf vendor

printf 'vendor.tar.xz for %s: %s bytes, %s crates\n' \
    "$stamp" \
    "$(wc -c < vendor.tar.xz)" \
    "$(tar tJf vendor.tar.xz | awk -F/ 'NF>1 && $2 != "" {print $2}' | sort -u | wc -l)"
