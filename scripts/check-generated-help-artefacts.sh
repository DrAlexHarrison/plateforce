#!/usr/bin/env bash
# The manual pages and the completion scripts are read by other programs, so this asks those
# programs rather than reading the files.
#
# A page that exists and a page `man` renders are different claims, and so are a completion
# script that parses and one that completes. Both weaker claims are already held by
# `cargo test`; what a file cannot tell anyone is whether the program it was written for
# accepts it, and that is the whole point of generating it.
#
# Exit 0 both accepted, 1 one of them rejected, 3 the tool that would decide is not here.

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

binary="${PLATEFORCE_BINARY:-}"
if [ -z "$binary" ]; then
    cargo build -q -p plateforce-cli || exit 3
    binary="$root/target/debug/plateforce"
fi
if [ ! -x "$binary" ]; then
    echo "plateforce: no runnable binary at $binary, so this check did not run" >&2
    exit 3
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export NO_COLOR=1

failures=0
unchecked=0

# The pages, asked of `man` itself. `-M` names the directory and `-P cat` keeps a pager from
# taking the terminal, which would hang this where it runs unattended.
if command -v man >/dev/null 2>&1; then
    "$binary" man --out-dir "$work/manual" >/dev/null || exit 3
    written=$(find "$work/manual/man1" -name 'plateforce*.1' | wc -l)
    if [ "$written" -lt 10 ]; then
        echo "plateforce: $written pages written, under a floor of 10, so this check saw too" \
             "little to be measuring anything" >&2
        exit 3
    fi
    for page in plateforce plateforce-analyse plateforce-methods plateforce-registry-show; do
        rendered=$(MANPAGER=cat MANWIDTH=80 man -M "$work/manual" -P cat "$page" 2>"$work/man.err")
        if [ -z "$rendered" ]; then
            echo "plateforce: man rendered nothing for $page" >&2
            sed -n '1,3p' "$work/man.err" | sed 's/^/    /' >&2
            failures=$((failures + 1))
            continue
        fi
        # The heading a reader looks for, and a flag spelled the way it is typed. roff refills
        # a paragraph and may hyphenate across a line break, which turns `--sentinel` into
        # something that does not run, so the spelling is what is asserted rather than the
        # presence of the word.
        if ! printf '%s' "$rendered" | grep -q 'EXAMPLES'; then
            echo "plateforce: $page carries no EXAMPLES section" >&2
            failures=$((failures + 1))
        fi
    done
    analyse=$(MANPAGER=cat MANWIDTH=80 man -M "$work/manual" -P cat plateforce-analyse 2>/dev/null)
    if ! printf '%s' "$analyse" | grep -q -- '--sentinel none --preset sams'; then
        echo "plateforce: the analyse page's example does not survive man's line breaking," \
             "so a reader copying it out gets a command that does not run" >&2
        failures=$((failures + 1))
    fi
    echo "man rendered 4 of $written pages"
else
    echo "plateforce: no man on this machine, so the pages were not put to it" >&2
    unchecked=$((unchecked + 1))
fi

# The completions, asked of the shell. bash calls a completion function with the command, the
# word being completed and the word before it, so it is called that way here.
if command -v bash >/dev/null 2>&1; then
    "$binary" completions bash >"$work/plateforce.bash" || exit 3
    offered=$(bash -c '
        set -u
        source "$1" || exit 1
        COMP_WORDS=(plateforce me); COMP_CWORD=1
        _plateforce plateforce me plateforce
        printf "%s\n" "${COMPREPLY[*]-}"
    ' _ "$work/plateforce.bash" 2>"$work/bash.err")
    if [ "$offered" != "methods" ]; then
        echo "plateforce: the bash completion offered '$offered' for 'me', where the one" \
             "command starting with those letters is 'methods'" >&2
        sed -n '1,3p' "$work/bash.err" | sed 's/^/    /' >&2
        failures=$((failures + 1))
    else
        echo "bash completed 'me' to 'methods'"
    fi
else
    unchecked=$((unchecked + 1))
fi

# zsh reads its script rather than being driven, because a completion there runs inside the
# completion system rather than as a function anyone can call.
if command -v zsh >/dev/null 2>&1; then
    "$binary" completions zsh >"$work/_plateforce" || exit 3
    if ! zsh -n "$work/_plateforce" 2>"$work/zsh.err"; then
        echo "plateforce: zsh will not parse its own completion script" >&2
        sed -n '1,3p' "$work/zsh.err" | sed 's/^/    /' >&2
        failures=$((failures + 1))
    else
        echo "zsh parsed its completion script"
    fi
else
    unchecked=$((unchecked + 1))
fi

if [ "$unchecked" -ge 3 ]; then
    echo "plateforce: neither man nor a shell is on this machine, so this run proves nothing" >&2
    exit 3
fi

if [ "$failures" -gt 0 ]; then
    echo "plateforce: $failures generated artefacts were rejected by the program that reads them" >&2
    exit 1
fi

echo "every generated artefact was accepted by the program that reads it"
