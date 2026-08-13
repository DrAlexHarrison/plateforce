#!/usr/bin/env bash
# Every command line the help shows is a command line that runs.
#
# An example that does not execute is worse than no example: a reader who cannot get the
# printed line to work has no way to tell whether they mistyped it or whether the program
# changed under it, and the second is what happens when a flag is renamed and the help below
# it is not.
#
# The lines are read out of the help the binary prints rather than out of the source that
# writes it, because the two are only the same while nobody has put an example anywhere else.
# One rule decides what a line is, stated in `src/examples.rs` and applied here: two spaces,
# then the program's name. Prose sits at column zero and never matches.
#
# A run either produces a result or declines with a published refusal code, and those are not
# the same thing. `decision_not_made` is the software answering, and an example that shows it
# is teaching the reader the choice the tool will not make for them. `command_line_not_parsed`
# is the example itself being wrong, and it carries the same exit status as the first, so the
# status alone cannot tell them apart and the code is read instead.

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

fixture="$root/crates/plateforce-conformance/fixtures/subject01_trial1.force.txt"
if [ ! -r "$fixture" ]; then
    echo "plateforce: no fixture at $fixture, so no example could be run" >&2
    exit 3
fi

# The folder an example is written against, holding the two names every example reads. A run
# here writes nothing into the machine's own manual, completion or configuration folders,
# which two of the examples would otherwise do.
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/trials" "$work/home"
cp "$fixture" "$work/jump.txt"
cp "$fixture" "$work/trials/AT01_1.force.txt"
cp "$fixture" "$work/trials/AT01_2.force.txt"
export HOME="$work/home"
export XDG_DATA_HOME="$work/home/.local/share"
export XDG_CONFIG_HOME="$work/home/.config"
export NO_COLOR=1

# Every page a reader can reach, both widths, because `-h` and `--help` show different
# blocks and an example shown in only one of them is still an example.
pages=("")
while read -r command; do
    [ -n "$command" ] || continue
    [ "$command" = "help" ] && continue
    [ "$command" = "serve" ] && continue
    pages+=("$command")
    while read -r nested; do
        [ -n "$nested" ] || continue
        [ "$nested" = "help" ] && continue
        pages+=("$command $nested")
    done < <("$binary" "$command" --help 2>/dev/null |
        sed -n '/^Commands:/,/^$/p' | sed -n 's/^  \([a-z][a-z-]*\) .*/\1/p')
done < <("$binary" --help 2>/dev/null |
    sed -n '/^Commands:/,/^$/p' | sed -n 's/^  \([a-z][a-z-]*\) .*/\1/p')

examples="$work/examples.txt"
: >"$examples"
for page in "${pages[@]}"; do
    for width in -h --help; do
        # shellcheck disable=SC2086
        "$binary" $page "$width" 2>/dev/null
    done
done | sed -n 's/^  \(plateforce .*\)$/\1/p' | sed 's/[[:space:]]*$//' | sort -u >"$examples"

found=$(wc -l <"$examples")

# The extractor is the part of this check most able to fail silently: a help page that stops
# printing examples, or a pattern that stops matching them, leaves a run with nothing to do
# and every assertion below satisfied. Measured against the pages this tree carries.
least_expected=12
if [ "$found" -lt "$least_expected" ]; then
    echo "plateforce: $found example lines found across ${#pages[@]} pages, under a floor of" \
         "$least_expected, so this check saw too little to be measuring anything" >&2
    exit 3
fi

ran=0
declined=0
broken=0

while IFS= read -r line; do
    # Split on whitespace with globbing off, so no brace, star or bracket in an example is
    # rewritten by this shell into something the reader would never have typed.
    set -f
    # shellcheck disable=SC2206
    words=($line)
    set +f

    (cd "$work" && "$binary" "${words[@]:1}" >"$work/out.txt" 2>"$work/err.txt")
    status=$?

    if [ "$status" -eq 0 ]; then
        ran=$((ran + 1))
        continue
    fi

    case "$status" in
        64 | 65 | 66 | 78)
            # The same command again, asked for the record rather than the sentence, because
            # the exit status is shared by the refusal that teaches and the one that means the
            # example is wrong.
            (cd "$work" && "$binary" "${words[@]:1}" --format json \
                >"$work/out.json" 2>"$work/err.json")
            code=$(sed -n 's/.*"code":"\([a-z_]*\)".*/\1/p' "$work/err.json" | head -1)
            if [ -z "$code" ]; then
                echo "plateforce: exited $status and published no refusal code, so nothing" \
                     "says whether it answered: $line" >&2
                broken=$((broken + 1))
            elif [ "$code" = "command_line_not_parsed" ]; then
                echo "plateforce: this line does not parse, so the help is showing a command" \
                     "that cannot be run: $line" >&2
                sed -n '1,2p' "$work/err.txt" | sed 's/^/    /' >&2
                broken=$((broken + 1))
            else
                declined=$((declined + 1))
            fi
            ;;
        *)
            echo "plateforce: exited $status, which is neither a result nor a published" \
                 "refusal: $line" >&2
            sed -n '1,2p' "$work/err.txt" | sed 's/^/    /' >&2
            broken=$((broken + 1))
            ;;
    esac
done <"$examples"

# Both controls. Without the first, a surface whose every example refused would pass; without
# the second, the branch that tells a teaching refusal from a broken line is never taken and
# could stop working without anything noticing.
if [ "$ran" -eq 0 ]; then
    echo "plateforce: 0 of $found examples produced a result, so this run measured no" \
         "working command" >&2
    exit 3
fi
if [ "$declined" -eq 0 ]; then
    echo "plateforce: 0 of $found examples reached the refusal branch, so the check that" \
         "tells a published refusal from an unparseable line did not run" >&2
    exit 3
fi

if [ "$broken" -gt 0 ]; then
    echo "plateforce: $broken of $found example lines do not run" >&2
    exit 1
fi

echo "$found of $found example lines run, across ${#pages[@]} help pages:" \
     "$ran produced a result, $declined declined by a published refusal"
