#!/usr/bin/env bash
#
# Assemble the published site: the download page at the root, the application one level in.
#
# A reader arriving with no context lands on the page that offers them the program, and a
# reader who wants the program is one link from it. Nothing under web/ moves to achieve that,
# so every check that serves web/ as its own root keeps measuring the thing it was written
# against.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-$here/site}"

# The application is the half of this that cannot be assembled from source files alone, and a
# site carrying a download page over an application that refuses to start is the worse of the
# two failures, so the bundle is required rather than assumed.
if [ ! -f "$here/web/pkg/plateforce_wasm.js" ]; then
  echo "web/pkg is not built; run scripts/build-web.sh first" >&2
  exit 3
fi

rm -rf "$out"
mkdir -p "$out"
cp "$here"/download/* "$out/"
cp -r "$here/web" "$out/app"

# The offer names files it has not read, so the two halves are proved to be present together
# rather than separately.
for required in index.html get.css get.js app/index.html app/styles.css; do
  if [ ! -f "$out/$required" ]; then
    echo "assembled site is missing $required" >&2
    exit 1
  fi
done

echo "site assembled at $out"
