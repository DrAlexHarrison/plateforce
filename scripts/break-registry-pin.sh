#!/usr/bin/env bash
# Reintroduces the defect this branch repaired, so the guard against it can be seen failing.
#
# A guard that has only ever been green proves nothing: it may be asserting something that
# cannot fail. So each break below is applied to the tree, the suite is run, and the failure
# is read. Every one asserts its anchor first and aborts on a miss, because a break that never
# applied and a break the guard survived print the same green line.
#
# Restores are `git checkout HEAD -- <file>`, never `git checkout -- <file>`: the second
# restores out of the index, which is a no-op when nothing was staged and silently discards
# uncommitted work when something was. Commit before running this.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [ -n "$(git status --porcelain)" ]; then
  echo "the tree is dirty, and this script restores from HEAD; commit first" >&2
  exit 1
fi

restore() {
  git checkout HEAD -- "$@"
  if [ -n "$(git status --porcelain -- "$@")" ]; then
    echo "restore of $* did not land" >&2
    exit 1
  fi
}

# name, file, the exact text to replace, what to replace it with.
apply() {
  local name="$1" file="$2" from="$3" to="$4"
  if ! grep -qF -- "$from" "$file"; then
    echo "anchor for '$name' is not in $file, so nothing was broken" >&2
    exit 1
  fi
  python3 - "$file" "$from" "$to" <<'PY'
import sys
path, before, after = sys.argv[1], sys.argv[2], sys.argv[3]
with open(path, encoding="utf-8") as handle:
    text = handle.read()
count = text.count(before)
if count != 1:
    raise SystemExit(f"the anchor appears {count} times in {path}, and one edit is intended")
with open(path, "w", encoding="utf-8") as handle:
    handle.write(text.replace(before, after))
PY
  echo "applied $name"
}

expect_red() {
  local name="$1"
  shift
  if "$@" > /tmp/break-registry-pin.log 2>&1; then
    echo "FAILED: '$name' is in the tree and the suite is still green" >&2
    tail -20 /tmp/break-registry-pin.log >&2
    exit 1
  fi
  echo "red under '$name', as it should be:"
  grep -E "panicked at|assertion|FAILED|failures:|Error|not ok" /tmp/break-registry-pin.log | head -6 || true
}

echo "=== 1. the terminal publishes the registry's claim as the caller's pin ==="
apply "terminal publishes the claim as the pin" \
  crates/plateforce-cli/src/analyse.rs \
  '    .pinned_to(args.registry_version.clone())' \
  '    .pinned_to(registry.declared_version.clone())'
expect_red "terminal publishes the claim as the pin" \
  cargo test -q -p plateforce-cli --test result_parity
restore crates/plateforce-cli/src/analyse.rs

echo
echo "=== 2. the pin never reaches the result ==="
apply "the pin is dropped" \
  crates/plateforce-cli/src/analyse.rs \
  '    .pinned_to(args.registry_version.clone())' \
  '    .pinned_to(None)'
expect_red "the pin is dropped" \
  cargo test -q -p plateforce-cli --test result_parity
restore crates/plateforce-cli/src/analyse.rs

echo
echo "=== 3. the registry's own claim stops travelling ==="
apply "the claim is dropped" \
  crates/plateforce-cli/src/analyse.rs \
  '        registry.declared_version.clone(),
        Some(registry.content_digest.clone()),' \
  '        None,
        Some(registry.content_digest.clone()),'
expect_red "the claim is dropped" \
  cargo test -q -p plateforce-cli --test result_parity
restore crates/plateforce-cli/src/analyse.rs

echo
echo "=== 4. an unpinned run omits the key rather than writing null ==="
# Anchored on the line above as well, because SpreadDocument in the same file declares a
# field spelled identically and the bare line matches both. The uniqueness check caught it.
apply "registry_version is omitted when absent" \
  crates/plateforce-analysis/src/document.rs \
  '    /// field, and a reader has no way to ask the document which happened.
    pub registry_version: Option<String>,' \
  '    /// field, and a reader has no way to ask the document which happened.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_version: Option<String>,'
expect_red "registry_version is omitted when absent" \
  cargo test -q -p plateforce-cli --test result_parity
restore crates/plateforce-analysis/src/document.rs

echo
# Read from the registry rather than written here. A digest literal in this file is one more
# provenance figure nobody checks, which is the defect section 5 exists to redden, and the
# guard caught this script committing exactly that.
answered=$(cargo run -q -p plateforce-cli -- --format json registry validate \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["ok"]["registry_digest"])')
if [ -z "$answered" ]; then
  echo "the registry named no digest, so sections 5 and 6 would assert nothing" >&2
  exit 1
fi
echo "this registry answers $answered"

echo
echo "=== 5. a worked example quotes a digest this registry does not answer ==="
# The wrong digest is built from the right one rather than written down, for the same reason
# the right one is not written down: a sixteen-hex literal in this file is a digest the guard
# would then have to be told to ignore.
stale="content-$(printf '%s' "${answered#content-}" | rev)"
if [ "$stale" = "$answered" ]; then
  echo "the reversed digest equals the real one, so this break changes nothing" >&2
  exit 1
fi
apply "the README quotes a stale digest" \
  crates/plateforce-python/README.md \
  "($answered)" \
  "($stale)"
expect_red "the README quotes a stale digest" \
  cargo test -q -p plateforce-cli --test digests_in_prose
restore crates/plateforce-python/README.md

echo
echo "=== 6. a file outside the list starts quoting a digest ==="
stray=docs/wsrp-a-file-that-quotes-a-digest.md
printf '%s\n' "$answered" > "$stray"
git add -N "$stray"
echo "applied a file outside the list quotes a digest"
expect_red "a file outside the list quotes a digest" \
  cargo test -q -p plateforce-cli --test digests_in_prose
# An untracked file is not restored by git checkout, which is the third of the four traps
# named at the top of this script. It is removed by name.
git rm -q --cached "$stray"
rm -f "$stray"
if [ -e "$stray" ]; then
  echo "the stray file is still here" >&2
  exit 1
fi

echo
echo "=== 7. the terminal pins nothing and the browser claims it did ==="
apply "one surface disagrees about the pin" \
  crates/plateforce-wasm/src/lib.rs \
  '                &plateforce_core::provenance::RegistryStamp::unpinned(
                    loaded.registry.declared_version.clone(),
                    Some(loaded.digest.clone()),
                ),' \
  '                &plateforce_core::provenance::RegistryStamp::unpinned(
                    loaded.registry.declared_version.clone(),
                    Some(loaded.digest.clone()),
                )
                .pinned_to(Some("wsrp-a-pin-no-caller-made".to_string())),'
expect_red "one surface disagrees about the pin" bash scripts/result-parity.sh --check
restore crates/plateforce-wasm/src/lib.rs
bash scripts/build-web.sh >/dev/null 2>&1

echo
echo "=== 8. one surface names a registry revision the others do not ==="
apply "one surface names another revision" \
  crates/plateforce-cli/src/analyse.rs \
  '        registry.declared_version.clone(),
        Some(registry.content_digest.clone()),' \
  '        Some("wsrp-a-revision-this-registry-does-not-name".to_string()),
        Some(registry.content_digest.clone()),'
expect_red "one surface names another revision" bash scripts/result-parity.sh --check
restore crates/plateforce-cli/src/analyse.rs

echo
echo "all eight breaks reddened their guard and every restore landed"
git status --porcelain
