#!/usr/bin/env bash
#
# Refuses a release that is missing a platform, or that carries a browser build this tree
# did not produce, or whose two version homes disagree.
#
#   ./scripts/verify-release-artefacts.sh <dist directory>   the whole declared set
#   ./scripts/verify-release-artefacts.sh --bundle-wasm-only  this platform's bundle payload
#
# Counted against a declared list rather than inspected one by one: a dropped build leg
# leaves the artefacts that did arrive perfectly valid, so a check reading only what is
# present reports a release missing a platform as sound.

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

readonly THE_MODULE="web/pkg/plateforce_wasm_bg.wasm"

# macOS ships `shasum` and no `sha256sum`, so a check written with one name reports exit 127
# on the platform it was meant to protect, and 127 arrives through the same door as a real
# mismatch. Absent both, this says so rather than comparing nothing.
digest_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -c1-64
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -c1-64
  else
    echo "no sha256 tool here, so nothing was compared" >&2
    exit 1
  fi
}

version_from() {
  cargo metadata --format-version 1 --no-deps --manifest-path "$1" 2>/dev/null \
    | python3 -c "
import json, sys
packages = json.load(sys.stdin)['packages']
wanted = sys.argv[1]
print(next(p['version'] for p in packages if p['name'] == wanted))" "$2"
}

# A bundle embeds the browser build inside its executable rather than shipping it as a file
# beside one, so there is nothing to extract and compare. The bundle reports the digest of
# the module it carries by executing itself, and that is what is compared here.
#
# A bundle that answers nothing and a bundle that answers a different digest are different
# faults, and reading an empty answer as a mismatched one sends a reader looking for a stale
# browser build in a bundle that carries none.
bundle_payload_digest() {
  local executable="$1"
  local reported status
  set +e
  reported="$("$executable" --capability)"
  status=$?
  set -e
  if [ "$status" -ne 0 ]; then
    echo "the bundle could not say what it carries, and said why above" >&2
    exit 1
  fi
  case "$reported" in
    sha256:????????????????????????????????????????????????????????????????) ;;
    *)
      echo "the bundle answered something that is not a digest: $reported" >&2
      exit 1
      ;;
  esac
  echo "${reported#sha256:}"
}

check_bundle_payload() {
  if [ ! -f "$THE_MODULE" ]; then
    echo "no browser build in the tree to compare against; run scripts/build-web.sh release" >&2
    exit 1
  fi
  local in_the_tree
  in_the_tree="$(digest_of "$THE_MODULE")"

  local executable=""
  local scratch=""
  case "$(uname -s)" in
    Linux)
      local package
      package="$(ls src-tauri/target/release/bundle/deb/*.deb 2>/dev/null | head -1 || true)"
      if [ -z "$package" ]; then
        echo "no deb was built, so there is no bundle payload to read" >&2
        exit 1
      fi
      scratch="$(mktemp -d)"
      dpkg-deb -x "$package" "$scratch"
      # The executable keeps the crate's name rather than taking the product name. On Linux
      # that is what stops it landing on PATH as `plateforce`, where it would collide with
      # the command line program of that name. The launcher entry reads plateforce.
      executable="$(find "$scratch" -type f -name 'plateforce-desktop' -perm -u+x | head -1)"
      ;;
    Darwin)
      executable="$(find src-tauri/target -type f -path '*plateforce.app/Contents/MacOS/*' | head -1)"
      ;;
    *)
      executable="src-tauri/target/release/plateforce-desktop.exe"
      ;;
  esac

  if [ -z "$executable" ] || [ ! -x "$executable" ]; then
    echo "the built bundle carries no runnable plateforce to ask" >&2
    [ -n "$scratch" ] && rm -rf "$scratch"
    exit 1
  fi

  local in_the_bundle
  in_the_bundle="$(bundle_payload_digest "$executable")"
  [ -n "$scratch" ] && rm -rf "$scratch"

  if [ "$in_the_bundle" != "$in_the_tree" ]; then
    echo "the bundle carries a browser build this tree did not produce" >&2
    echo "  web/pkg: sha256 $in_the_tree" >&2
    echo "  bundle:  sha256 $in_the_bundle" >&2
    exit 1
  fi
  echo "bundle wasm matches web/pkg: sha256 $in_the_bundle"
}

# Four manifests state the version and this used to compare two of them, so the R package
# and the R crate could name a different release from the desktop and the terminal and
# nothing said so. The comparison lives in one place now, and the tag is compared against
# the same four at the top of the release route.
check_versions() {
  python3 scripts/verify-version-homes.py
}

# The install documentation is the only place the routes are stated, so a command it names
# and the binary does not have is a false instruction to the one population that cannot fall
# back on a desktop artefact. Read off the document rather than listed here, so a route added
# to it later is covered without this being edited.
check_documented_commands() {
  local binary="${1}/plateforce-x86_64-linux-static"
  if [ ! -f "$binary" ]; then
    echo "no static binary to ask, so the documented routes are unchecked" >&2
    return 1
  fi
  # A zipped artefact comes back without its executable bit, so the file arrives whole and
  # will not run. Saying it is absent sends a reader looking for a build that did happen.
  if [ ! -x "$binary" ]; then
    echo "the static binary is here and not executable, so the documented routes are" >&2
    echo "unchecked. Whatever carried it dropped the executable bit." >&2
    return 1
  fi

  local usage missing=()
  usage="$("$binary" --help 2>&1)"
  while read -r command; do
    [ -z "$command" ] && continue
    printf '%s' "$usage" | command grep -qw -- "$command" || missing+=("$command")
  done < <(command grep -oE 'linux-static [a-z][a-z-]*' docs/install.md | awk '{print $2}' | sort -u)

  if [ "${#missing[@]}" -gt 0 ]; then
    echo "docs/install.md names commands this binary does not have: ${missing[*]}" >&2
    return 1
  fi
  echo "every route docs/install.md names is one the binary answers"
}

# Naming a command in the help text is not answering it. This runs one, from a directory
# holding nothing, which is what a reader who downloaded a single file has.
#
# The population this artefact exists for cannot fetch a registry: enterprise Linux, an
# air-gapped analysis box, a machine whose administrator is somebody else. A binary that
# lists `registry` in its help and then exits 78 because no `registry/` directory sits in
# the working directory reaches them with a terminal that cannot answer anything, and the
# check above passes it, because the word is in the help.
check_terminal_reaches_the_registry() {
  local binary
  binary="$(cd "$1" && pwd)/plateforce-x86_64-linux-static"
  [ -x "$binary" ] || { echo "no runnable static binary to ask" >&2; return 1; }

  local sandbox reported status
  sandbox="$(mktemp -d)"
  set +e
  reported="$(cd "$sandbox" && "$binary" registry census 2>&1)"
  status=$?
  set -e
  rmdir "$sandbox" 2>/dev/null

  if [ "$status" -ne 0 ]; then
    echo "the terminal cannot reach a registry with none beside it, exit ${status}:" >&2
    printf '  %s\n' "$reported" >&2
    return 1
  fi
  # A census that counted nothing exits zero and reads like a working one, so the count is
  # read rather than the exit code alone. Anchored, because the rows below this one break
  # the same population down and carry their own numbers.
  if ! printf '%s' "$reported" | command grep -qE '^ *computation entries +[1-9]'; then
    echo "the terminal answered with no computation entries:" >&2
    printf '  %s\n' "$reported" >&2
    return 1
  fi
  echo "the terminal reaches a registry with none beside it: $(printf '%s' "$reported" | head -1)"
}

check_declared_set() {
  local dist="$1"
  local version
  version="$(version_from Cargo.toml plateforce-cli)"

  # Declared here and counted against, never inferred from what turned up. The MSIX is
  # absent on purpose: it goes to the Store rather than to a release page.
  local declared=(
    "plateforce_${version}_amd64.AppImage"
    "plateforce_${version}_amd64.deb"
    "plateforce-${version}-1.x86_64.rpm"
    "plateforce_${version}_universal.dmg"
    "plateforce_${version}_x64-setup.exe"
    "plateforce-x86_64-linux-static"
    "plateforce-aarch64-linux-static"
  )

  local present=0 missing=()
  for name in "${declared[@]}"; do
    if [ -f "${dist}/${name}" ]; then
      present=$((present + 1))
    else
      missing+=("$name")
    fi
  done

  echo "${present} of ${#declared[@]} declared artefacts present"
  if [ "${#missing[@]}" -gt 0 ]; then
    printf '  missing: %s\n' "${missing[@]}" >&2
    return 1
  fi
}

case "${1:-}" in
  --bundle-wasm-only)
    check_bundle_payload
    ;;
  "")
    echo "usage: $0 <dist directory> | --bundle-wasm-only" >&2
    exit 2
    ;;
  *)
    status=0
    check_declared_set "$1" || status=1
    check_versions || status=1
    check_documented_commands "$1" || status=1
    check_terminal_reaches_the_registry "$1" || status=1
    exit "$status"
    ;;
esac
