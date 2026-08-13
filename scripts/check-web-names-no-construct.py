#!/usr/bin/env python3
"""No construct the registry declares is written down where a reader meets it.

Two scans here, and a third that this file cannot run. A construct id reaches a reader by
three routes and each is invisible to the others' method.

The rail is built from the rules the build declares it can run and labelled from the
registry's own entries, so a construct id appearing as a literal in one of the modules that
builds that model is a row the registry can no longer add or remove without an edit here.
Adding a method is a data edit, and this is the half of that ruling the interface owes.

The second route is the registry's own prose. Those strings are written in no module; they
arrive while the page is running, so a construct id inside one is on screen while the module
scan passes and says so. That is how one shipped: a reader adding jump height from the picker
met "Not comparable with standing_frame without a declared correction" under the row's title,
with 337 literals checked and clean.

The third route is a sentence composed in Rust that crosses as JSON while the page is running
and is rendered verbatim. It is written in no module and in no registry field, so both scans
below pass clean while the id is on screen. That is how one shipped: the panel a folder run
opens when its choices are still to be made printed `system_weight` and `movement_onset` at
the reader, two clicks from a rail calling the same two quantities "Standing still, before the
jump" and "Start of the jump". It needs a running page, so it is asked in
`scripts/check-batch.mjs`, under "a run held open for a choice spells no construct the registry
declares".

The chart is not scanned. It draws the three landmark tracks the response carries index
fields for, and those field names are the engine's, not the registry's.

Run with no arguments. Exits non-zero naming each name and where it sits.
"""

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

# The registry fields a person reads as words rather than as data.
READER_FACING = ('label', 'title', 'notes')

QUOTED = re.compile(r"'([^'\\\n]*)'|\"([^\"\\\n]*)\"")


def constructs() -> list:
    with open('registry/constructs.toml', 'rb') as handle:
        return tomllib.load(handle).get('construct', [])


def identifier_forms(declared: set) -> set:
    """The spellings of a construct that no English sentence produces.

    An id and its final dotted segment, kept only where the token carries an underscore or a
    dot. `takeoff` and `landing` are construct ids and are also ordinary words, and a pattern
    matching those reports 21 hits here of which 19 are sentences: "Elapsed time from movement
    onset to takeoff" is not a leaked identifier. A count taken with that pattern reads exactly
    like a count taken with this one, which is the reason the distinction is written down here
    rather than left inside whoever wrote the expression.
    """
    forms = set()
    for identifier in declared:
        for token in (identifier, identifier.rsplit('.', 1)[-1]):
            if '_' in token or '.' in token:
                forms.add(token)
    return forms


def main() -> int:
    rows = constructs()
    declared = {row['id'] for row in rows}
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

    forms = identifier_forms(declared)
    strings = [
        (row['id'], field, row[field])
        for row in rows
        for field in READER_FACING
        if row.get(field)
    ]
    leaked = []
    for owner, field, text in strings:
        for token in forms:
            if re.search(r'(?<![\w.])' + re.escape(token) + r'(?![\w.])', text):
                leaked.append(
                    f'  constructs.toml {owner}.{field} spells the identifier {token}: {text}'
                )

    # Every floor is asserted rather than the absence of hits, because a scan that stopped
    # reading and a tree with nothing to find print the same zero.
    if literals < 50:
        print(f'{literals} string literals found across {len(MODEL)} modules, so the scan matched nothing')
        return 1
    if len(strings) < 100 or len(forms) < 40:
        print(
            f'{len(strings)} reader-facing registry strings against {len(forms)} identifier '
            'forms, so the registry scan matched nothing'
        )
        return 1

    for line in sorted(set(named)) + sorted(set(leaked)):
        print(line)
    print(
        f'{literals} string literals across {len(MODEL)} interface modules, '
        f'checked against {len(declared)} declared constructs, {len(set(named))} written down'
    )
    print(
        f'{len(strings)} reader-facing registry strings across {len(READER_FACING)} fields, '
        f'checked against {len(forms)} identifier forms, {len(set(leaked))} spelling one'
    )
    return 1 if named or leaked else 0


if __name__ == '__main__':
    sys.exit(main())
