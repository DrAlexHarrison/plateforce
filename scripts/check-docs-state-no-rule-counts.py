#!/usr/bin/env python3
"""No reader-facing document states a count of rules or entries in prose.

CONVENTIONS section 4 makes this rule for README.md: capability is output the software
prints, `capability` and `registry census`, never prose. The same number in docs/ goes stale
the same way and nothing reruns a sentence. A count that moves as the work lands is a query.

Run from the repository root.
"""
import re
import sys

PAGES = [
    "README.md",
    "docs/terminal.md",
    "docs/for-an-agent.md",
    "docs/r-surface.md",
    "docs/schema.md",
    "web/README.md",
    "bindings/r/README.md",
    "crates/plateforce-python/README.md",
]

WORDS = "one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve"
POPULATIONS = "rules?|methods?|computation entries|protocol entries|preset entries|constructs?|refusal codes?|operations?"
CLAIM = re.compile(r"\b(?:%s|\d+)\s+(?:%s)\b" % (WORDS, POPULATIONS), re.I)

# Sentences where the number is a property of the design rather than a population that
# grows: how many flags exist, how many shapes a construct's rules may take.
EXEMPT = re.compile(
    r"one rule per construct"
    r"|one rule instead of two"
    r"|two methods of one construct"
    r"|two entries"
    r"|three rules govern"
    r"|three rules keep"
    r"|one rule in full"
    r"|three steps have a flag"
    r"|implementations of this one rule",
    re.I,
)

found = []
for page in PAGES:
    try:
        lines = open(page, encoding="utf-8", errors="replace").read().splitlines()
    except FileNotFoundError:
        sys.exit("%s is named by this check and does not exist" % page)
    for number, line in enumerate(lines, 1):
        if EXEMPT.search(line):
            continue
        hit = CLAIM.search(line)
        if hit:
            found.append((page, number, hit.group(0), line.strip()))

print("pages checked: %d" % len(PAGES))
if found:
    print()
    for page, number, claim, line in found:
        print("  %s:%d  states '%s'" % (page, number, claim))
        print("      %s" % line[:150])
    sys.exit(
        "%d prose sentence(s) state a population the software already prints" % len(found)
    )
print("no page states a rule or entry count in prose")
