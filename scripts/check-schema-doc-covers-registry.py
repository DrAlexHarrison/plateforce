#!/usr/bin/env python3
"""Every key and table the shipped registry uses appears in a docs/schema.md TOML block.

docs/schema.md calls itself the contract every crate and every binding is written against.
A field the registry carries and the document omits is a contract with a hole in it, and the
hole is invisible to a reader of either file alone. Run from the repository root.
"""
import glob
import re
import sys

SCHEMA = "docs/schema.md"

# Keys the registry uses that no schema block shows on purpose, each with the reason it is
# absent from the contract rather than missing from it.
EXEMPT = {
    # `n` is the sample count inside a bias figure's own example prose, shown in text
    # rather than in a fenced block.
    "n",
}


def keys_and_tables(text):
    keys = set(re.findall(r"^\s*([a-z][a-z0-9_]*)\s*=", text, re.M))
    tables = {"[" + t + "]" for t in re.findall(r"^\s*\[\[?([a-z][a-z0-9_.]*)\]\]?", text, re.M)}
    return keys | tables


document = open(SCHEMA, encoding="utf-8", errors="replace").read()
documented = set()
for block in re.findall(r"```toml\n(.*?)```", document, re.S):
    documented |= keys_and_tables(block)

registry_files = sorted(glob.glob("registry/**/*.toml", recursive=True))
if not registry_files:
    sys.exit("no registry files found, so this check compared nothing")

used = set()
for path in registry_files:
    used |= keys_and_tables(open(path, encoding="utf-8", errors="replace").read())

undocumented = sorted(used - documented - EXEMPT)

print(
    "registry files %d, keys and tables in use %d, shown in %s %d"
    % (len(registry_files), len(used), SCHEMA, len(documented))
)
if undocumented:
    print()
    for name in undocumented:
        where = [p for p in registry_files if name.strip("[]") in open(p, encoding="utf-8", errors="replace").read()]
        print("  %-28s used in %s" % (name, ", ".join(where[:3])))
    sys.exit(
        "%s shows no example carrying %d of the %d keys and tables the registry uses"
        % (SCHEMA, len(undocumented), len(used))
    )
print("every key and table the registry uses is shown in an example")
