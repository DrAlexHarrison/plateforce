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
#
# `BuildInfo` is here because the browser asks it which of three maps a construct travels in
# and then writes the request accordingly. A field renamed there does not render an empty
# string, it sends a construct under the wrong name and the engine refuses the whole request,
# which is the loudest failure any of these three can produce and was the least guarded.
#
# `state.analysis` holds a `ResultDocument` and not the `AnalysisResponse` inside it. The two
# share thirteen field names, so every read the browser had made until now resolved against
# either and this scan was checking the wrong declaration: it would have reported a field only
# the document carries as gone, and passed a field only the response carries, which never
# crosses the boundary at all.
# `BoundGlobal` is the row for a value the analysis was bound to rather than any one rule, and
# the Analysis record renders one per row. It carries the claim about where the value came
# from, so a field renamed here blanks the line that tells a stated gravity from the constant
# nobody was asked about, which is the one thing that row exists to say.
RECORDS = {
    'BoundMethod': 'crates/plateforce-analysis/src/resolution.rs',
    'BoundGlobal': 'crates/plateforce-analysis/src/response.rs',
    'ResultDocument': 'crates/plateforce-analysis/src/document.rs',
    'BuildInfo': 'crates/plateforce-wasm/src/lib.rs',
}

# How a record is spelled in `web/`, as the variable the field hangs off. Anchored on a word
# boundary so `boundEntry.surfacing`, a registry candidate rather than a record, is not read
# as one.
#
# The build reaches two spellings: off the tab as `state.build`, and as the argument named
# `build` that the decision model is handed. One pattern covers both, and it needs the dot,
# so `buildDecisionModel` and `buildRequest` are not read as reads.
READERS = {
    'BoundMethod': r'\bbound(?:\?)?\.(\w+)',
    'BoundGlobal': r'\bboundGlobal(?:\?)?\.(\w+)',
    'ResultDocument': r'\bstate\.analysis(?:\?)?\.(\w+)',
    'BuildInfo': r'\bbuild(?:\?)?\.(\w+)',
}

def serialised_fields(path: str, struct: str) -> set:
    """The field names a struct puts on the wire, dropping the ones serde is told to skip.

    Visibility is not read, on the struct or on its fields. What serde writes is decided by
    its own attributes and never by `pub`, so a private field is on the wire exactly like a
    public one and a scanner that demands `pub` reports a record it cannot see as a record
    that does not exist.
    """
    source = open(path).read()
    opening = re.search(rf'^(?:pub )?struct {struct} \{{$', source, re.M)
    if not opening:
        raise SystemExit(f'{path} declares no struct named {struct}')
    body = source[opening.end():source.index('\n}\n', opening.end())]

    fields, skip_next, renamed = set(), False, None
    attribute = ''
    for line in body.splitlines():
        stripped = line.strip()
        if attribute or stripped.startswith('#['):
            attribute = f'{attribute} {stripped}'.strip()
            if attribute.count('[') != attribute.count(']'):
                continue
            if re.search(r'serde\([^)]*\bskip\b\s*[,)]', attribute):
                skip_next = True
            rename = re.search(r'serde\([^)]*rename\s*=\s*"([^"]+)"', attribute)
            if rename:
                renamed = rename.group(1)
            attribute = ''
            continue
        declared = re.match(r'(?:pub )?(\w+):', stripped)
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
