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

# The rail's slot titles, the trace legend and every notice heading are JavaScript literals
# rather than markup, and the words that fail this rule are in them, so the check reaches
# them where they are written rather than only where they are easy to parse.
#
# Matched over the whole file rather than line by line. A call wrapped across lines reads
# exactly like a file with nothing to find, and two of these were being missed that way.
JAVASCRIPT_LABELS = (
    (Path("web/registry.js"), r"\btitle: '([^']*)'", "a slot title"),
    (Path("web/workspace.js"), r"\['var\(--[a-z-]+\)',\s*'([^']*)'\]", "a legend entry"),
)

# Every notice heading in the browser, found by what it is rather than by a list of files
# that would go stale the first time somebody writes one somewhere new.
NOTICE_HEADING = r"\bnotice\(\s*'[a-z]+',\s*'([^']*)'"
notice_sources = sorted(path for path in Path("web").glob("*.js"))

blind_patterns = []
for source, pattern, what in JAVASCRIPT_LABELS:
    if not source.exists():
        continue
    text = source.read_text()
    found = list(re.finditer(pattern, text))
    if not found:
        blind_patterns.append(f"{source} for {what}")
    for match in found:
        if not match.group(1).strip():
            continue
        number = text.count("\n", 0, match.start()) + 1
        labelled.append((f"{source}:{number}", match.group(1).strip()))

notices = 0
for source in notice_sources:
    text = source.read_text()
    for match in re.finditer(NOTICE_HEADING, text):
        if not match.group(1).strip():
            continue
        notices += 1
        number = text.count("\n", 0, match.start()) + 1
        labelled.append((f"{source}:{number}", match.group(1).strip()))
if not notices:
    blind_patterns.append(f"{len(notice_sources)} files in web/ for a notice heading")

vocabulary_failures = 0
for where, text in labelled:
    hit = banned.search(identifier.sub("", text))
    if hit:
        vocabulary_failures += 1
        report(where, f'"{text}" uses "{hit.group(0)}", which appears in 0 of 6 teaching documents')

# A pattern that silently matches nothing reads exactly like a file with nothing to find, so
# every pattern has to see something and the markup has to yield a string named here.
control = [text for _, text in labelled if text == "Drop a force trace here"]
if not control:
    report("rule 2", f"read {len(labelled)} labels and no control string, so a pass would mean nothing")
for blind in blind_patterns:
    report("rule 2", f"read nothing from {blind}, so it is not checking what it claims to")
if control and not blind_patterns and not vocabulary_failures:
    print(f"pass  rule 2, {len(labelled)} headings, labels, legends, buttons and notices "
          f"use the words the audience uses")


# --------------------------------------------------- 3. strings about the software

ABOUT_THE_SOFTWARE = (
    "in this build", "not implemented yet", "coming soon", "build default",
    "available here", "listed disabled", "generated in this tab",
)
about = re.compile("|".join(re.escape(phrase) for phrase in ABOUT_THE_SOFTWARE), re.IGNORECASE)
state_hits = 0
scanned = 0
read = set()
for path in sorted(Path("web").rglob("*")):
    if path.is_dir() or path.name == "pkg" or "pkg" in path.parts or path.suffix not in {".html", ".js", ".css"}:
        continue
    scanned += 1
    read.add(path.as_posix())
    for number, line in enumerate(path.read_text().splitlines(), start=1):
        hit = about.search(line)
        if hit:
            state_hits += 1
            report(f"{path}:{number}", f'"{hit.group(0)}" describes the state of the software')

# This rule can only ever pass on its own evidence, so it carries its own, taken from a
# document it does not own. Rules 1 and 2 read real content and a break shows up in it; this
# one reads the absence of content, where a dead pattern and a clean interface are the same
# reading.
#
# The list is checked against CONVENTIONS.md section 5 in both directions, because each
# direction catches a different failure: a phrase here that section 5 does not ban is a
# check enforcing something nobody agreed, and a phrase section 5 bans that nothing here
# matches is a ban with no enforcement. A probe built out of this tuple would match itself
# whatever it said, which is a control that cannot fail.
# The paragraph rather than the section: section 5 also quotes a sentence as an illustration
# of the class, and a parse that swept the whole section would read the illustration as an
# item on the list. Whitespace is normalised inside the paragraph because the list wraps
# across lines, and a pattern that does not tolerate the wrap undercounts silently.
paragraphs = Path("CONVENTIONS.md").read_text().split("\n\n")
listed = [" ".join(block.split()) for block in paragraphs if "Banned outright:" in block]
banned_in_conventions = [phrase for block in listed for phrase in re.findall(r'"([^"]+)"', block)]

control_failures = 0
if not banned_in_conventions:
    control_failures += 1
    report("rule 3", "CONVENTIONS.md yielded no banned list, so this rule is checking itself")
unagreed = [phrase for phrase in ABOUT_THE_SOFTWARE
            if not any(phrase.lower() in quoted.lower() for quoted in banned_in_conventions)]
if unagreed:
    control_failures += 1
    report("rule 3", f"{len(unagreed)} of {len(ABOUT_THE_SOFTWARE)} phrases are not in CONVENTIONS.md "
                     f"section 5's banned list, so this check enforces something nobody agreed: {unagreed}")

unenforced = [quoted for quoted in banned_in_conventions
              if not about.search(quoted)]
if unenforced:
    control_failures += 1
    report("rule 3", f"{len(unenforced)} of {len(banned_in_conventions)} phrases CONVENTIONS.md section 5 "
                     f"bans are matched by nothing here: {unenforced}")

# The scan reaching the file the copy is written in, named rather than counted, so narrowing
# the glob cannot read as an interface that stopped saying these things.
CARRIES_THE_COPY = "web/index.html"
if CARRIES_THE_COPY not in read:
    control_failures += 1
    report("rule 3", f"read {scanned} files and not {CARRIES_THE_COPY}, which is where the copy is")

if not state_hits and not control_failures:
    print(f"pass  rule 3, no string across {scanned} files in web/ describes the state of the software "
          f"({len(ABOUT_THE_SOFTWARE)} phrases, each on CONVENTIONS.md section 5's list of "
          f"{len(banned_in_conventions)} and each matching it)")


for failure in failures:
    print(f"FAIL  {failure}")
if failures:
    print(f"\n{len(failures)} failures")
    sys.exit(1)
print("\nthree rules, no failures")
CHECKS
