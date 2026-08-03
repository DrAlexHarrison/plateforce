#!/usr/bin/env python3
"""No construct the registry declares is written down in the interface.

The rail is built from the rules the build declares it can run and labelled from the
registry's own entries, so a construct id appearing as a literal in one of the modules that
builds that model is a row the registry can no longer add or remove without an edit here.
Adding a method is a data edit, and this is the half of that ruling the interface owes.

The chart is not scanned. It draws the three landmark tracks the response carries index
fields for, and those field names are the engine's, not the registry's.

Run with no arguments. Exits non-zero naming each literal and the module it sits in.
"""

import glob
import os
import re
import sys
import tomllib

# The modules that decide which rows exist, what each row is called, and what the request
# names. A construct written into any of them is a row that cannot come from data.
MODEL = [
    'web/registry.js',
    'web/decisions.js',
    'web/analysis.js',
    'web/startup.js',
    'web/workspace.js',
    'web/batch-run.js',
    'web/add-quantity.js',
    'web/state.js',
]

QUOTED = re.compile(r"'([^'\\\n]*)'|\"([^\"\\\n]*)\"")


def construct_ids() -> set:
    with open('registry/constructs.toml', 'rb') as handle:
        return {row['id'] for row in tomllib.load(handle).get('construct', [])}


def main() -> int:
    declared = construct_ids()
    if len(declared) < 40:
        print(f'the registry yielded {len(declared)} constructs, so the scan has nothing to look for')
        return 1

    named, literals = [], 0
    for path in MODEL:
        if not os.path.exists(path):
            print(f'  {path} is listed here and is not in the tree')
            return 1
        source = open(path).read()
        for single, double in QUOTED.findall(source):
            literals += 1
            written = single or double
            if written in declared:
                named.append(f'  {os.path.basename(path)} writes down the construct {written}')

    # A regex that stopped matching reports zero violations exactly as a clean tree does, so
    # the floor is asserted rather than the absence of hits.
    if literals < 50:
        print(f'{literals} string literals found across {len(MODEL)} modules, so the scan matched nothing')
        return 1

    for line in sorted(set(named)):
        print(line)
    print(
        f'{literals} string literals across {len(MODEL)} interface modules, '
        f'checked against {len(declared)} declared constructs, {len(set(named))} written down'
    )
    return 1 if named else 0


if __name__ == '__main__':
    sys.exit(main())
