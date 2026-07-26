#!/usr/bin/env bash
#
# Builds the browser bundle into web/pkg/.
#
# wasm-bindgen-cli rather than wasm-pack, for one reason: the CLI version and the
# wasm-bindgen crate version have to match exactly, and pinning one tool is easier to
# reproduce than pinning a wrapper that chooses it. The version below is read from
# Cargo.toml so the two can never drift.
#
#   ./scripts/build-web.sh          debug, fast
#   ./scripts/build-web.sh release  optimised
#
# Then serve the static directory. Opening index.html from the filesystem will not work,
# because browsers refuse to instantiate WebAssembly from file:// URLs:
#
#   python3 -m http.server -d web 8000

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

profile="${1:-debug}"
target_dir="target/wasm32-unknown-unknown/${profile}"
output_dir="web/pkg"

wasm_bindgen_version="$(
  sed -n 's/^wasm-bindgen = .*version = "=\([0-9.]*\)".*/\1/p' crates/plateforce-wasm/Cargo.toml
)"
if [ -z "$wasm_bindgen_version" ]; then
  echo "could not read the pinned wasm-bindgen version from crates/plateforce-wasm/Cargo.toml" >&2
  exit 1
fi

if ! rustup target list --installed | grep -qx wasm32-unknown-unknown; then
  echo "installing the wasm32-unknown-unknown target"
  rustup target add wasm32-unknown-unknown
fi

installed_version="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
if [ "$installed_version" != "$wasm_bindgen_version" ]; then
  echo "installing wasm-bindgen-cli ${wasm_bindgen_version} (found '${installed_version:-none}')"
  cargo install wasm-bindgen-cli --version "$wasm_bindgen_version" --locked
fi

echo "building plateforce-wasm for wasm32-unknown-unknown (${profile})"
if [ "$profile" = "release" ]; then
  cargo build --package plateforce-wasm --target wasm32-unknown-unknown --release
else
  cargo build --package plateforce-wasm --target wasm32-unknown-unknown
fi

rm -rf "$output_dir"
wasm-bindgen \
  --target web \
  --no-typescript \
  --out-dir "$output_dir" \
  "${target_dir}/plateforce_wasm.wasm"

# Deliberately no wasm-opt pass. Binaryen 108, which is what Ubuntu ships, rewrites the
# function table in a way that makes wasm-bindgen's module fail to instantiate with
# "WebAssembly.Table.grow(): failed to grow table by 4". A post-processing step whose
# behaviour depends on whichever binaryen the machine happens to have is a worse trade
# than a larger artifact, and the release profile already carries opt-level 2 and thin LTO.

size="$(du -h "${output_dir}/plateforce_wasm_bg.wasm" | cut -f1)"
transfer="$(gzip -c "${output_dir}/plateforce_wasm_bg.wasm" | wc -c | awk '{printf "%.0f KB", $1/1024}')"
echo "built ${output_dir}/plateforce_wasm_bg.wasm (${size} on disk, ${transfer} gzipped over the wire)"
echo "serve with: python3 -m http.server -d web 8000"
