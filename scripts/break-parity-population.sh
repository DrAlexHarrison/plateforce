#!/usr/bin/env bash
# Put back, one at a time, each defect the population gate was written to catch, and require
# the gate to redden. A green gate proves nothing unless it has been watched failing.
#
# Each case asserts its anchor in the source before touching anything and aborts if the anchor
# is not there, because a substitution that matched nothing reads exactly like a case the gate
# passed. Each restores through `git checkout HEAD --`, which is the form that reverts a staged
# or unstaged edit, and asserts the restore landed rather than assuming it.
#
# The two source cases rebuild a surface, so this takes upwards of a quarter of an hour. It is
# not a CI gate. Run it after any change to what a surface does with a sample it cannot read,
# or to how a surface claims its parameters came to be:
#
#     bash scripts/break-parity-population.sh [case]
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
wanted="${1:-}"

if [ -n "$(git status --porcelain)" ]; then
    echo "the tree carries uncommitted work, and every case here restores through git" >&2
    exit 1
fi

failed=0

# Runs the gate and requires it to fail naming `expected`. A case whose gate failed for some
# other reason is reported as a case that proved nothing.
#
# Every search here forces text mode. `grep` on this machine routes to a ugrep that classifies
# input holding one invalid multibyte sequence as binary and returns no lines, in silence.
# Demonstrated on this shell: a here-string carrying the expected sentence plus two stray bytes
# returns not-found, and the same query against the same sentence alone returns found, so the
# control says the query works and the miss is the binary classification. The gate's own output
# is ASCII today, `file` reports "ASCII text" on a full green run, so this is a guard rather
# than a repair. It matters because a truncation anywhere upstream can slice a multi-byte
# character in half, which is how a lead's all-gates run read green over a red gate.
requires_red() {
    local case_name="$1" expected="$2" output status
    set +o errexit
    output="$(./scripts/result-parity.sh --check 2>&1)"
    status=$?
    set -o errexit
    if [ "$status" -eq 0 ]; then
        echo "  NOT CAUGHT: the gate passed with $case_name applied" >&2
        failed=1
        return
    fi
    if ! grep -aqF -- "$expected" <<< "$output"; then
        echo "  CAUGHT, BUT NOT BY THIS: nothing in the failure mentions '$expected'" >&2
        echo "$output" | tail -12 | sed 's/^/      /' >&2
        failed=1
        return
    fi
    echo "  red: $(grep -aF -- "$expected" <<< "$output" | head -1 | cut -c1-140)"
}

# `git checkout HEAD --` rather than `git checkout --`, which restores from the index and would
# hand back a staged edit instead of reverting it.
restore() {
    git checkout HEAD -- "$@"
    if [ -n "$(git status --porcelain "$@")" ]; then
        echo "  THE RESTORE DID NOT LAND for $*, so nothing after this means anything" >&2
        exit 1
    fi
}

assert_anchor() {
    local file="$1" anchor="$2"
    # Text mode for the reason above, and it bites harder here: a source file this reads as
    # binary would report its anchor absent, and the case would abort claiming the edit had
    # already landed.
    if ! grep -aqF -- "$anchor" "$file"; then
        echo "the anchor is not in $file, so this case would change nothing: $anchor" >&2
        exit 1
    fi
}

control() {
    echo "applied nothing, the control"
    set +o errexit
    ./scripts/result-parity.sh --check > /dev/null 2>&1
    local status=$?
    set -o errexit
    if [ "$status" -ne 0 ]; then
        echo "  THE CONTROL DOES NOT PASS, so no case below means anything" >&2
        exit 1
    fi
    echo "  green: the population passes untouched"
}

# The browser holding an unreadable sample at the last real reading, which made its answer to
# the interrupted recording identical to its answer to the intact one.
case_the_tab_repairs_the_recording() {
    local file="crates/plateforce-wasm/src/lib.rs"
    local anchor="let treated_as_missing = flagged.map_or(0, |dropped| dropped.len());"
    assert_anchor "$file" "$anchor"
    python3 - "$file" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
was = """        let treated_as_missing = flagged.map_or(0, |dropped| dropped.len());

        let trial =
            Trial::new(column.to_vec(), sample_rate_hz).map_err(|e| JsError::new(&e.to_string()))?;"""
now = """        let mut treated_as_missing = 0usize;
        let mut force = Vec::with_capacity(column.len());
        for (index, value) in column.iter().enumerate() {
            let missing = !value.is_finite()
                || flagged
                    .as_ref()
                    .is_some_and(|dropped| dropped.binary_search(&index).is_ok());
            if missing {
                treated_as_missing += 1;
                force.push(force.last().copied().unwrap_or(0.0));
            } else {
                force.push(*value);
            }
        }

        let trial =
            Trial::new(force, sample_rate_hz).map_err(|e| JsError::new(&e.to_string()))?;"""
if was not in text:
    raise SystemExit("the block to replace is not there verbatim")
path.write_text(text.replace(was, now, 1), encoding="utf-8")
PY
    echo "applied the tab holding an unreadable sample at the last real reading"
    requires_red "the tab holding samples" "interrupted: browser does not match"
    restore "$file"
}

# Python posting the registry's own defaults as the caller's stated values.
case_python_claims_the_registry_s_defaults() {
    local file="crates/plateforce-python/src/analysis.rs"
    local anchor="from_registry_default,
        ..Default::default()"
    assert_anchor "$file" "from_registry_default,"
    python3 - "$file" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
was = """        options: options.unwrap_or_default(),
        manual_index: None,
        from_registry_default,
        ..Default::default()"""
now = """        options: options.unwrap_or_default(),
        manual_index: None,
        from_registry_default: Default::default(),
        ..Default::default()"""
if was not in text:
    raise SystemExit("the block to replace is not there verbatim")
path.write_text(text.replace(was, now, 1), encoding="utf-8")
PY
    echo "applied Python claiming the registry's defaults as the caller's own"
    requires_red "Python claiming defaults" "inverted: python does not match"
    restore "$file"
}

# A request file committed and named by no row, which is how two of them sat here for a day.
case_a_request_nobody_asks() {
    local file="scripts/result-parity-requests.txt"
    assert_anchor "$file" "inverted	tests/golden/result-parity-request-inverted.json"
    grep -v '^inverted	' "$file" > "$file.cut" && mv "$file.cut" "$file"
    echo "applied a committed request file that no row of the manifest names"
    requires_red "a request nobody asks" "asks a question and no row of"
    restore "$file"
}

CASES="the_tab_repairs_the_recording python_claims_the_registry_s_defaults a_request_nobody_asks"

control
for name in $CASES; do
    if [ -n "$wanted" ] && [ "$wanted" != "$name" ]; then continue; fi
    "case_$name"
done

echo "applied nothing again, the restore"
set +o errexit
./scripts/result-parity.sh --check > /dev/null 2>&1
status=$?
set -o errexit
if [ "$status" -ne 0 ]; then
    echo "  THE TREE DID NOT COME BACK GREEN, so a case left something behind" >&2
    exit 1
fi
echo "  green again"

exit "$failed"
