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

sh "$here/ensure-engine.sh"

stamp=$(sed -n 's/^Version: *//p' "$package_root/DESCRIPTION" | head -1)

cd "$crate"
rm -rf vendor vendor.tar.xz vendor.tar vendor.members
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

# One mtime and one member order, expressed through touch and a sorted member list
# rather than through --mtime and --sort, which macOS's tar does not carry.
find vendor -exec touch -t 202601010000 {} +
find vendor -print | LC_ALL=C sort > vendor.members

# The container holds a hard link's target in a field ustar caps at 100 bytes, and a
# vendored crate ships test fixtures linked past it. pax holds them and records an access
# and a change time to the nanosecond, which no two runs share. The two tars spell this one
# container differently, so the name is probed rather than read off a version string.
if tar --format=gnu -cf /dev/null --files-from /dev/null 2>/dev/null; then
    container=gnu
else
    container=gnutar
fi

# A compressor succeeds on empty input, so a tar read through a pipe leaves a file that
# reads as a bundle whatever tar did. Each step is taken on its own status, and the
# bundle takes its name only once it is whole.
tar -cf vendor.tar --format="$container" --numeric-owner -T vendor.members
xz -T0 -9 < vendor.tar > vendor.tar.part
mv -f vendor.tar.part vendor.tar.xz
rm -rf vendor vendor.tar vendor.members

printf 'vendor.tar.xz for %s: %s bytes, %s crates\n' \
    "$stamp" \
    "$(wc -c < vendor.tar.xz)" \
    "$(tar tJf vendor.tar.xz | awk -F/ 'NF>1 && $2 != "" {print $2}' | sort -u | wc -l)"
