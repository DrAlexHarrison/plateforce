#!/usr/bin/env bash
# Every byte the terminal surface writes by default is ASCII.
#
# `cmd.exe` under a raster font mangles box drawing, so a rule drawn with hyphens reaches a
# reader that a rule drawn with box characters does not. `render.rs` states that policy and
# nothing measured it, which left it holding for exactly as long as every author remembered
# it.
#
# The commands are run rather than read. A glyph reaches a terminal through whatever the
# code path emits, which includes a dependency's error text and a character the formatter
# chose, and none of those is visible to a search of this repository's own source.
#
# Both streams are read, because a refusal sentence is output too and is the line most
# likely to carry a typographic quotation mark.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

binary="${PLATEFORCE_BINARY:-}"
if [ -z "$binary" ]; then
    cargo build -q -p plateforce-cli
    binary="$root/target/debug/plateforce"
fi

if [ ! -x "$binary" ]; then
    echo "plateforce: no runnable binary at $binary, so this check did not run" >&2
    exit 3
fi

trial=crates/plateforce-conformance/fixtures/subject01_trial1.force.txt

# Each entry is a label and the arguments. The set spans a document, a manifest, the
# registry's three verbs and two refusals, because the refusal sentence is written by hand
# and the document is written by the formatter.
runs=(
    "version|version"
    "capability|capability"
    "census|registry census"
    "validate|registry validate"
    "show|registry show takeoff.threshold.absolute_force"
    "analyse|analyse $trial"
    "refuse-unknown-id|registry show no.such.entry.exists"
    "refuse-missing-file|analyse /nonexistent/trace.txt"
)

# A byte floor per run. A command that prints nothing is trivially ASCII, so without this
# a broken invocation reads exactly like a clean one. This is the control, and it must be
# large enough that an error message alone cannot satisfy it for the document runs.
declare -A floor=(
    [version]=5 [capability]=200 [census]=40 [validate]=10 [show]=100
    [analyse]=200 [refuse-unknown-id]=10 [refuse-missing-file]=10
)

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

violations=0
unchecked=0

for entry in "${runs[@]}"; do
    label="${entry%%|*}"
    args="${entry#*|}"

    # A refusal is an expected exit code rather than a failure to check, so the status is
    # recorded and only a signal or a missing binary counts as unable to check.
    set +e
    # shellcheck disable=SC2086
    "$binary" $args >"$work/$label.out" 2>"$work/$label.err"
    status=$?
    set -e

    cat "$work/$label.out" "$work/$label.err" >"$work/$label.all"
    bytes=$(wc -c <"$work/$label.all")

    if [ "$status" -ge 126 ]; then
        echo "plateforce: $label could not run, exit $status. This check did not measure it" >&2
        unchecked=$((unchecked + 1))
        continue
    fi

    if [ "$bytes" -lt "${floor[$label]}" ]; then
        echo "plateforce: $label wrote $bytes bytes against a floor of ${floor[$label]}," \
             "so this check saw too little to be measuring anything" >&2
        unchecked=$((unchecked + 1))
        continue
    fi

    # grep -P counts the offending lines; the byte offsets name the first one for a reader.
    if LC_ALL=C grep -qP '[^\x00-\x7F]' "$work/$label.all"; then
        count=$(LC_ALL=C grep -cP '[^\x00-\x7F]' "$work/$label.all")
        first=$(LC_ALL=C grep -boP '[^\x00-\x7F]+' "$work/$label.all" | head -1)
        echo "plateforce: $label emitted a non-ASCII byte on $count line(s), first at offset ${first%%:*}" >&2
        LC_ALL=C grep -nP '[^\x00-\x7F]' "$work/$label.all" | head -3 | sed 's/^/    /' >&2
        violations=$((violations + 1))
    fi
done

if [ "$unchecked" -gt 0 ]; then
    echo "plateforce: $unchecked of ${#runs[@]} commands were not measured, so this run proves nothing" >&2
    exit 3
fi

if [ "$violations" -gt 0 ]; then
    echo "plateforce: $violations of ${#runs[@]} commands emitted a non-ASCII byte in default output" >&2
    exit 1
fi

echo "${#runs[@]} of ${#runs[@]} commands emitted ASCII only"
