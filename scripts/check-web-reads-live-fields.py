#!/usr/bin/env python3
"""Every field the browser reads off an engine record is a field that record serialises.

A field renamed in Rust and migrated across the Rust callers leaves the browser reading
`undefined`, which renders as an empty string rather than as an error, so the page keeps
working and quietly stops saying the thing it was written to say. That is invisible to the
module-link gate, which checks imports between JavaScript files and never crosses the wire.

Run with no arguments. Exits non-zero naming each field that no longer exists.
"""

import re
import sys

# The records the browser destructures by name, and where each one's fields are declared.
RECORDS = {
    'BoundMethod': 'crates/plateforce-analysis/src/resolution.rs',
    'AnalysisResponse': 'crates/plateforce-analysis/src/response.rs',
}

# How a record is spelled in `web/`, as the variable the field hangs off. Anchored on a word
# boundary so `boundEntry.surfacing`, a registry candidate rather than a record, is not read
# as one.
READERS = {
    'BoundMethod': r'\bbound(?:\?)?\.(\w+)',
    'AnalysisResponse': r'\bstate\.analysis(?:\?)?\.(\w+)',
}

# Names reached on the record as an object rather than as a declared field.
OBJECT_METHODS = {'hasOwnProperty'}


def serialised_fields(path: str, struct: str) -> set:
    """The field names a struct puts on the wire, dropping the ones serde is told to skip."""
    source = open(path).read()
    opening = re.search(rf'^pub struct {struct} \{{$', source, re.M)
    if not opening:
        raise SystemExit(f'{path} declares no struct named {struct}')
    body = source[opening.end():source.index('\n}\n', opening.end())]

    fields, skip_next, renamed = set(), False, None
    for line in body.splitlines():
        stripped = line.strip()
        if stripped.startswith('#['):
            if re.search(r'serde\([^)]*\bskip\b\s*[,)]', stripped):
                skip_next = True
            rename = re.search(r'serde\([^)]*rename\s*=\s*"([^"]+)"', stripped)
            if rename:
                renamed = rename.group(1)
            continue
        declared = re.match(r'pub (\w+):', stripped)
        if not declared:
            continue
        if not skip_next:
            fields.add(renamed or declared.group(1))
        skip_next, renamed = False, None
    return fields


def main() -> int:
    import glob
    import os

    unknown, checked = [], 0
    for struct, path in RECORDS.items():
        declared = serialised_fields(path, struct)
        if len(declared) < 3:
            print(f'  {path} yielded {len(declared)} fields for {struct}, so it parsed nothing')
            return 1
        for source_path in sorted(glob.glob('web/*.js')):
            source = open(source_path).read()
            for read in re.findall(READERS[struct], source):
                if read in OBJECT_METHODS:
                    continue
                checked += 1
                if read not in declared:
                    unknown.append(
                        f'  {os.path.basename(source_path)} reads {read} off {struct}, '
                        f'which serialises {", ".join(sorted(declared))}'
                    )

    for line in unknown:
        print(line)
    # A pattern that stopped matching reports zero violations exactly as a clean tree does,
    # so the floor is asserted rather than the absence of hits.
    if checked < 10:
        print(f'{checked} reads found across {len(RECORDS)} records, so the scan matched nothing')
        return 1
    print(f'{checked} reads across {len(RECORDS)} engine records, {len(unknown)} naming a field that is gone')
    return 1 if unknown else 0


if __name__ == '__main__':
    sys.exit(main())
