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
output_dir="web/pkg"

# Where cargo will actually put the module, asked rather than assumed. CARGO_TARGET_DIR and
# a build.target-dir in config.toml both move it, and a literal `target/` then names a path
# that never appears, so the build succeeds and this script reports the module missing.
cargo_target_dir="$(
  cargo metadata --format-version 1 --no-deps 2>/dev/null \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])'
)"
if [ -z "$cargo_target_dir" ]; then
  echo "cargo did not say where it builds, so there is nowhere to read the module from" >&2
  exit 1
fi
target_dir="${cargo_target_dir}/wasm32-unknown-unknown/${profile}"

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

# Destroyed before the build rather than after it. `web/pkg/` is gitignored, so a bundle
# survives every run and every checkout, and four browser gates serve `web/` while
# `web/startup.js` imports `./pkg/plateforce_wasm.js`: a crate that fails to compile leaves
# `check-minute`, `check-grammar`, `check-batch` and `check-spread-scope` green against a
# bundle built from a tree nobody is testing.
#
# `r-surface.sh` clears its private library first and `install-python-wheel.sh` drops its
# installed package first, for the same reason: a producer that deletes before it builds leaves
# nothing when it fails, and its consumers fail loudly instead of answering from the last good run.
rm -rf "$output_dir"

echo "building plateforce-wasm for wasm32-unknown-unknown (${profile})"
if [ "$profile" = "release" ]; then
  cargo build --package plateforce-wasm --target wasm32-unknown-unknown --release
else
  # A debug bundle runs several times slower than the deployed one and is the same
  # filename, so anything timed against it measures the profile rather than the code.
  echo "  a debug bundle is not what the site serves; pass 'release' before timing anything"
  cargo build --package plateforce-wasm --target wasm32-unknown-unknown
fi

rm -rf "$output_dir"
wasm-bindgen \
  --target web \
  --no-typescript \
  --out-dir "$output_dir" \
  "${target_dir}/plateforce_wasm.wasm"

# No wasm-opt pass. Binaryen 108, the version Ubuntu ships, rewrites the function table
# so the module fails to instantiate with "WebAssembly.Table.grow(): failed to grow table
# by 4". The release profile already carries opt-level 2 and thin LTO.

size="$(du -h "${output_dir}/plateforce_wasm_bg.wasm" | cut -f1)"
transfer="$(gzip -c "${output_dir}/plateforce_wasm_bg.wasm" | wc -c | awk '{printf "%.0f KB", $1/1024}')"
echo "built ${output_dir}/plateforce_wasm_bg.wasm (${size} on disk, ${transfer} gzipped over the wire)"
echo "serve with: python3 -m http.server -d web 8000"
