#!/usr/bin/env bash
#
# Builds the static plateforce binaries, one per architecture, for machines that will not
# let you install anything: enterprise Linux, which never shipped webkit2gtk-4.1 and which
# no desktop artefact reaches, and air-gapped boxes, where a browser refuses to instantiate
# WebAssembly from a file:// URL.
#
#   ./scripts/build-serve-binaries.sh                         both musl targets
#   ./scripts/build-serve-binaries.sh <target> [<target>...]   named targets
#
# A dynamically linked output is refused here rather than on a researcher's RHEL box, where
# it fails with a glibc version message that says nothing about what to do.

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

if [ "$#" -gt 0 ]; then
  targets=("$@")
else
  targets=(x86_64-unknown-linux-musl aarch64-unknown-linux-musl)
fi

output_directory="dist"
mkdir -p "$output_directory"

# The binaries carry web/ and web/pkg/ compiled in, and web/pkg/ is a build artefact, so a
# bundle built after this point would not be the one inside them.
echo "building the browser bundle these binaries carry"
./scripts/build-web.sh release >/dev/null

for target in "${targets[@]}"; do
  if ! rustup target list --installed | grep -qx "$target"; then
    echo "installing the $target target"
    rustup target add "$target"
  fi

  # rustc ships musl's libc.a and its self-contained CRT objects, so neither leg needs a C
  # toolchain. The aarch64 leg needs its linker named: the host ld reads the cross-compiled
  # CRT objects as the wrong format and reports it as a generic ELF relocation error.
  if [ "$target" = "aarch64-unknown-linux-musl" ]; then
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
      cargo build --release --package plateforce-cli --target "$target"
  else
    cargo build --release --package plateforce-cli --target "$target"
  fi

  # Where cargo actually put it, asked rather than assumed. A contributor with
  # CARGO_TARGET_DIR set builds successfully and then meets "linkage could not be read",
  # which reads like a broken binary and is a script looking in the wrong directory.
  binary="${CARGO_TARGET_DIR:-target}/${target}/release/plateforce"
  described="$(file -b "$binary")"

  case "$described" in
    *dynamically\ linked*)
      echo "$target: dynamically linked, so it needs a glibc the target machine may not have" >&2
      echo "  file said: $described" >&2
      exit 1
      ;;
    # musl links x86_64 as a position-independent static executable and aarch64 as a plain
    # static one. Both are static; only the wording differs.
    *static-pie\ linked* | *statically\ linked*) ;;
    *)
      echo "$target: linkage could not be read from the built binary" >&2
      echo "  file said: $described" >&2
      exit 1
      ;;
  esac

  architecture="${target%%-*}"
  # No extension. The file a researcher copies onto a machine should look like a program.
  named="${output_directory}/plateforce-${architecture}-linux-static"
  cp -f "$binary" "$named"

  printf '%s %s bytes statically linked -> %s\n' \
    "$target" "$(stat -c %s "$named")" "$named"
done
