#!/usr/bin/env bash
#
# Three rules about the words a reader meets, checked mechanically.
#
# 1. The debate count is not the opening claim and never appears without its denominator.
#    "119 genuine methodological debates" reads to a first-year as a statement about their
#    qualifications. The consequence is the claim: ten published ways of computing one jump
#    height disagree by more than the training effect the source study was built to detect.
#    The count is read from the registry rather than written here, so the check follows the
#    data instead of going stale against it.
#
# 2. The words a first-year does not use do not appear in anything they read as a label.
#    Across six university course documents `onset`, `threshold`, `epoch`, `filter`,
#    `provenance` and `fingerprint` each appear in 0 of 6. They are legal as registry
#    identifiers, which are shown deliberately and on the same row as the label, and they
#    are illegal in the label itself.
#
# 3. No interface string describes the state of the software. `CONVENTIONS.md` section 5.
#    This one covers `web/` only: coverage limits belong in README.md, where somebody
#    deciding whether to adopt this looks for them.
#
# Usage: scripts/check-framing-copy.sh

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

# Rule 1 takes the count from the registry rather than carrying one, so it needs the
# command to run. When it cannot, say which command and why rather than exiting on its
# status, which reads as this check being broken.
if ! census="$(cargo run -q -p plateforce-cli -- registry census 2>&1)"; then
  printf 'FAIL  registry census could not run, so the debate count rule has no count to check\n'
  printf '%s\n' "$census" | tail -5
  exit 1
fi

python3 - "$census" <<'CHECKS'
import re
import sys
from pathlib import Path

census = sys.argv[1]
failures = []


def report(where, message):
    failures.append(f"{where} {message}")


# ------------------------------------------------------------------ 1. the count

match = re.search(r"genuine debates\s+(\d+) of (\d+)", census)
if match:
    debates, entries = match.group(1), match.group(2)
    bare = re.compile(rf"(?<!\d){debates}(?!\d)")
    checked = 0
    count_failures = 0
    for path in (Path("README.md"), Path("web/index.html")):
        if not path.exists():
            continue
        for number, line in enumerate(path.read_text().splitlines(), start=1):
            if not bare.search(line):
                continue
            checked += 1
            if f"of {entries}" not in line:
                count_failures += 1
                report(f"{path}:{number}", f"the debate count {debates} appears without its denominator: {line.strip()}")
    opening = "\n".join(Path("README.md").read_text().split("\n\n")[:2]) if Path("README.md").exists() else ""
    if bare.search(opening):
        count_failures += 1
        report("README.md", f"opens on the debate count {debates} rather than on what it costs the reader")
    if not count_failures:
        print(f"pass  rule 1, the debate count carries its denominator wherever it appears "
              f"({checked} occurrences checked, {debates} of {entries} read from registry census)")
else:
    report("registry census", "did not report the debate count, so rule 1 cannot be checked")


# ------------------------------------------------------------- 2. the vocabulary

NOT_THEIR_WORD = ("onset", "threshold", "epoch", "filter", "provenance", "fingerprint")
banned = re.compile(rf"\b({'|'.join(NOT_THEIR_WORD)})\b", re.IGNORECASE)
# A registry identifier is shown on purpose, on the same row as the label, so a word
# inside one is not the label using it.
identifier = re.compile(r"\b[a-z_]+(\.[a-z_0-9]+)+\b")

labelled = []

markup = Path("web/index.html")
for number, line in enumerate(markup.read_text().splitlines(), start=1):
    for tag, text in re.findall(r"<(h[1-4]|label|legend|button|summary)\b[^>]*>([^<]+)", line):
        if text.strip():
            labelled.append((f"{markup}:{number}", text.strip()))

# The rail's slot titles and the trace legend are JavaScript literals rather than markup,
# and the words that fail this rule are in them, so the check reaches them where they are
# written rather than only where they are easy to parse.
for source, pattern in (
    (Path("web/registry.js"), r"^\s+title: '([^']*)'"),
    (Path("web/workspace.js"), r"^\s+\['var\(--[a-z-]+\)', '([^']*)'\]"),
):
    if not source.exists():
        continue
    for number, line in enumerate(source.read_text().splitlines(), start=1):
        found = re.match(pattern, line)
        if found and found.group(1).strip():
            labelled.append((f"{source}:{number}", found.group(1).strip()))

vocabulary_failures = 0
for where, text in labelled:
    hit = banned.search(identifier.sub("", text))
    if hit:
        vocabulary_failures += 1
        report(where, f'"{text}" uses "{hit.group(0)}", which appears in 0 of 6 teaching documents')

# A pattern that silently matches nothing reads exactly like a file with nothing to find,
# so the rule refuses to pass on zero strings, and one string it must always see is named.
control = [text for _, text in labelled if text == "Drop a force trace here"]
if not control:
    report("rule 2", f"read {len(labelled)} labels and no control string, so a pass would mean nothing")
elif not vocabulary_failures:
    print(f"pass  rule 2, {len(labelled)} headings, labels, legends and buttons use the words the audience uses")


# --------------------------------------------------- 3. strings about the software

ABOUT_THE_SOFTWARE = (
    "in this build", "not implemented yet", "coming soon", "build default",
    "available here", "listed disabled", "generated in this tab",
)
about = re.compile("|".join(re.escape(phrase) for phrase in ABOUT_THE_SOFTWARE), re.IGNORECASE)
state_hits = 0
scanned = 0
for path in sorted(Path("web").rglob("*")):
    if path.is_dir() or path.name == "pkg" or "pkg" in path.parts or path.suffix not in {".html", ".js", ".css"}:
        continue
    scanned += 1
    for number, line in enumerate(path.read_text().splitlines(), start=1):
        hit = about.search(line)
        if hit:
            state_hits += 1
            report(f"{path}:{number}", f'"{hit.group(0)}" describes the state of the software')
if not state_hits:
    print(f"pass  rule 3, no string across {scanned} files in web/ describes the state of the software")


for failure in failures:
    print(f"FAIL  {failure}")
if failures:
    print(f"\n{len(failures)} failures")
    sys.exit(1)
print("\nthree rules, no failures")
CHECKS
