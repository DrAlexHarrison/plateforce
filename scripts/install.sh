#!/bin/sh
# Fetches the plateforce command line program for this machine and puts it somewhere the
# shell will find it.
#
#   curl -fsSL https://raw.githubusercontent.com/DrAlexHarrison/plateforce/main/scripts/install.sh | sh
#
# Reads no input, so it runs the same way inside a pipe, a container and a continuous
# integration job. Writes one file, prints where it went, and verifies the program answers.
set -eu

owner_and_repo="DrAlexHarrison/plateforce"
release="https://github.com/$owner_and_repo/releases/latest/download"

# The desktop application is a separate download because it carries a browser engine and a
# window. This script fetches the program a terminal runs.
say() { printf '%s\n' "$*"; }
fail() { printf 'plateforce install: %s\n' "$*" >&2; exit 1; }

kernel="$(uname -s)"
machine="$(uname -m)"

case "$kernel" in
    Darwin)
        # One file holds both architectures, so a Mac of either kind takes the same download.
        asset="plateforce-universal-macos"
        ;;
    Linux)
        case "$machine" in
            x86_64 | amd64) asset="plateforce-x86_64-linux-static" ;;
            aarch64 | arm64) asset="plateforce-aarch64-linux-static" ;;
            *) fail "$machine is not a machine this program is built for. Building from source is described at https://github.com/$owner_and_repo" ;;
        esac
        ;;
    MINGW* | MSYS* | CYGWIN*)
        fail "on Windows, download plateforce-x86_64-windows.exe from $release, or run this from Windows Subsystem for Linux"
        ;;
    *)
        fail "$kernel is not a system this program is built for. Building from source is described at https://github.com/$owner_and_repo"
        ;;
esac

# Somewhere on PATH that does not need a password. A machine where none of these exists gets
# the first one made, which is the conventional place for a program a single person installs.
for candidate in "${PLATEFORCE_INSTALL_DIR:-}" "$HOME/.local/bin" "$HOME/bin"; do
    [ -n "$candidate" ] || continue
    if [ -d "$candidate" ] && [ -w "$candidate" ]; then
        destination="$candidate"
        break
    fi
done
if [ -z "${destination:-}" ]; then
    destination="$HOME/.local/bin"
    mkdir -p "$destination"
fi

if command -v curl > /dev/null 2>&1; then
    fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget > /dev/null 2>&1; then
    fetch() { wget -q "$1" -O "$2"; }
else
    fail "neither curl nor wget is on this machine, and one of them fetches the download"
fi

# Downloaded beside the destination rather than into it, so an interrupted fetch cannot leave
# a half-written file where the shell would run it.
temporary="$(mktemp "$destination/.plateforce.XXXXXX")"
trap 'rm -f "$temporary"' EXIT INT TERM

say "Fetching $asset"
fetch "$release/$asset" "$temporary" || fail "the download did not complete. $release/$asset"

chmod 755 "$temporary"
version="$("$temporary" version 2>/dev/null || true)"
case "$version" in
    plateforce\ *) : ;;
    *) fail "the downloaded file did not answer as this program. Report it at https://github.com/$owner_and_repo/issues" ;;
esac

mv -f "$temporary" "$destination/plateforce"
trap - EXIT INT TERM

say ""
say "$version"
say "Installed to $destination/plateforce"

case ":${PATH}:" in
    *":$destination:"*)
        say "Run plateforce --help to begin."
        ;;
    *)
        say ""
        say "$destination is not on this shell's PATH. Add it with:"
        say ""
        say "  echo 'export PATH=\"$destination:\$PATH\"' >> ~/.profile"
        say ""
        say "Then open a new terminal, or run $destination/plateforce --help now."
        ;;
esac
