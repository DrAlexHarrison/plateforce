#!/usr/bin/env bash
# Prove a change moved no number the browser can see.
#
# Runs `examples/surface-dump.rs` against the working tree and against a git ref, and
# diffs the two outputs. The harness is copied into the worktree rather than taken from
# it, so the measuring instrument is the same on both sides and only the code under it
# differs. Refactors that move this code between crates are settled here rather than by
# reading the diff that caused them.
#
# Usage: scripts/prove-surface-unchanged.sh [ref]     ref defaults to HEAD

set -euo pipefail

readonly REF="${1:-HEAD}"
readonly REPO_ROOT="$(git rev-parse --show-toplevel)"
readonly HARNESS="crates/plateforce-wasm/examples/surface-dump.rs"
readonly WORK="$(mktemp -d)"
readonly BASELINE_TREE="${WORK}/baseline"

cleanup() {
    git -C "${REPO_ROOT}" worktree remove --force "${BASELINE_TREE}" >/dev/null 2>&1 || true
    command rm -rf "${WORK}"
}
trap cleanup EXIT

if [ ! -f "${REPO_ROOT}/${HARNESS}" ]; then
    echo "the harness is missing at ${HARNESS}" >&2
    exit 1
fi

echo "recording the working tree"
( cd "${REPO_ROOT}" && cargo run --quiet --example surface-dump -p plateforce-wasm ) \
    > "${WORK}/working-tree.txt"

echo "recording ${REF}"
git -C "${REPO_ROOT}" worktree add --detach --quiet "${BASELINE_TREE}" "${REF}"
mkdir -p "${BASELINE_TREE}/$(dirname "${HARNESS}")"
cp -f "${REPO_ROOT}/${HARNESS}" "${BASELINE_TREE}/${HARNESS}"
( cd "${BASELINE_TREE}" && cargo run --quiet --example surface-dump -p plateforce-wasm ) \
    > "${WORK}/baseline.txt"

readonly BASELINE_BYTES=$(wc -c < "${WORK}/baseline.txt")
readonly WORKING_BYTES=$(wc -c < "${WORK}/working-tree.txt")

if diff -q "${WORK}/baseline.txt" "${WORK}/working-tree.txt" >/dev/null; then
    echo
    echo "identical: ${BASELINE_BYTES} bytes from ${REF}, ${WORKING_BYTES} from the working tree"
    exit 0
fi

echo
echo "the browser surface moved between ${REF} and the working tree"
echo "${BASELINE_BYTES} bytes from ${REF}, ${WORKING_BYTES} from the working tree"
echo
# Truncated by writing the whole difference and reading the first lines back, rather than by
# piping into `head`. Under `set -o pipefail` a pipe into `head` returns 141 the moment `head`
# has read its fill and closes the pipe, so a real difference is answered with SIGPIPE instead
# of the exit 1 written below it, and a caller reading the status learns nothing about what it
# found.
diff "${WORK}/baseline.txt" "${WORK}/working-tree.txt" > "${WORK}/difference.txt" || true
readonly DIFFERENCE_LINES=$(wc -l < "${WORK}/difference.txt")
sed -n '1,80p' "${WORK}/difference.txt"
if [ "${DIFFERENCE_LINES}" -gt 80 ]; then
    echo "... ${DIFFERENCE_LINES} lines of difference in all, the first 80 shown"
fi
exit 1
