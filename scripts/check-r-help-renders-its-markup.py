#!/usr/bin/env python3
"""No R help page shows the markup its author wrote instead of what it means.

The roxygen sources are written in markdown: `[pf_value()]` is a cross-reference and
`` `x` `` is code. Without `Roxygen: list(markdown = TRUE)` in DESCRIPTION, roxygen2 copies
both through verbatim, and the help page a reader opens shows square brackets where a link
belongs and backticks where code formatting belongs.

Run from the repository root.
"""
import glob
import re
import sys

MAN = "bindings/r/man/*.Rd"
DESCRIPTION = "bindings/r/DESCRIPTION"

# A bracketed token that reads as a roxygen cross-reference rather than as R code. Rd's own
# syntax never produces one, and a real subscript in Rd is written \code{x[1]}.
LINK = re.compile(r"(?<![\w\\}])\[[A-Za-z_][A-Za-z0-9_.]*(?:\(\))?\](?![\w(])")
CODE = re.compile(r"`[^`\n]+`")
# A bare \link{topic} whose target is not a documented alias in this package fails R CMD
# check with "Missing link(s)".
BARE_LINK = re.compile(r"\\link\{([^}]+)\}")

pages = sorted(glob.glob(MAN))
if not pages:
    sys.exit("no man pages found, so this check compared nothing")

description = open(DESCRIPTION, encoding="utf-8", errors="replace").read()
if "Roxygen: list(markdown = TRUE)" not in description:
    print("%s does not turn roxygen markdown on" % DESCRIPTION)

aliases = set()
for page in pages:
    aliases |= set(re.findall(r"^\\alias\{([^}]+)\}", open(page, encoding="utf-8", errors="replace").read(), re.M))

faults = []
for page in pages:
    text = open(page, encoding="utf-8", errors="replace").read()
    for hit in LINK.finditer(text):
        faults.append((page, text[: hit.start()].count("\n") + 1, "cross-reference shown as markup", hit.group(0)))
    for hit in CODE.finditer(text):
        faults.append((page, text[: hit.start()].count("\n") + 1, "code span shown as markup", hit.group(0)))
    for hit in BARE_LINK.finditer(text):
        if hit.group(1) not in aliases:
            faults.append((page, text[: hit.start()].count("\n") + 1, "link to a topic this package does not document", hit.group(0)))

print("help pages checked: %d, documented topics: %d" % (len(pages), len(aliases)))
if faults:
    print()
    for page, line, what, snippet in faults[:40]:
        print("  %s:%d  %s: %s" % (page, line, what, snippet))
    if len(faults) > 40:
        print("  ... and %d more" % (len(faults) - 40))
    sys.exit("%d place(s) across %d pages show markup rather than what it means"
             % (len(faults), len({f[0] for f in faults})))
print("every page renders its cross-references and code spans")
