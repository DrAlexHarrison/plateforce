#!/usr/bin/env python3
"""Every name one browser module imports is a name the module it names exports.

A split that leaves an import dangling loads as a blank page with one console line, so this
runs as a gate rather than as a screen check. The generated wasm bindings are read from
disk like any other target, so a renamed export fails here rather than at runtime.

Three things have to be true before a zero here means every import resolved: the readers
have to work, they have to have read something, and the generated bindings have to be on
disk. A missing build is the loudest of the three, because it removes the one target whose
names nobody writes by hand, which is the class this claims to catch.
"""

import glob
import os
import re
import sys

IDENTIFIER = r'[\w$]+'
BINDINGS = 'web/pkg/plateforce_wasm.js'
IMPORT = rf"import\s+(?:{IDENTIFIER}\s*,\s*)?\{{([^}}]*)\}}\s*from\s*'([^']+)'"


def exported_names(source: str) -> set:
    names = set(re.findall(rf'^export\s+(?:async\s+)?function\s+({IDENTIFIER})', source, re.M))
    names |= set(re.findall(rf'^export\s+(?:const|let|var|class)\s+({IDENTIFIER})', source, re.M))
    for block in re.findall(r'^export\s*\{([^}]*)\}', source, re.M):
        names |= {t.strip().split(' as ')[-1].strip() for t in block.split(',') if t.strip()}
    return names


def imported_names(source: str) -> list:
    return [
        (os.path.basename(target),
         [t.strip().split(' as ')[0].strip() for t in block.split(',') if t.strip()])
        for block, target in re.findall(IMPORT, source)
    ]


# Samples these readers own, carrying one of every shape they claim to read. A reader that
# stopped reading a shape returns fewer names, and a module using that shape then reads as a
# module exporting nothing, which is indistinguishable from an import that genuinely
# resolves nowhere. The expectations are written out rather than computed from the samples.
CONTROL_MODULE = """
export function drawn() {}
export async function later() {}
export const held = 1;
export let counted = 2;
export class Rail {}
export { inner as outer, plain };
"""
CONTROL_EXPORTS = {'drawn', 'later', 'held', 'counted', 'Rail', 'outer', 'plain'}
CONTROL_IMPORTER = """
import init, { one, two as three } from './pkg/plateforce_wasm.js';
import { four } from './state.js';
"""
CONTROL_IMPORTS = [('plateforce_wasm.js', ['one', 'two']), ('state.js', ['four'])]


def readers_work() -> list:
    """What the two readers get wrong on samples written to exercise every shape they read."""
    faults = []
    found = exported_names(CONTROL_MODULE)
    if found != CONTROL_EXPORTS:
        faults.append(f'the export reader found {sorted(found)} in its own control sample '
                      f'where {sorted(CONTROL_EXPORTS)} is written into it')
    seen = imported_names(CONTROL_IMPORTER)
    if seen != CONTROL_IMPORTS:
        faults.append(f'the import reader found {seen} in its own control sample '
                      f'where {CONTROL_IMPORTS} is written into it')
    return faults


def main() -> int:
    faults = readers_work()
    for fault in faults:
        print(f'  {fault}')

    exports, imports = {}, {}
    for path in sorted(glob.glob('web/*.js')):
        source = open(path).read()
        name = os.path.basename(path)
        exports[name] = exported_names(source)
        imports[name] = imported_names(source)

    bindings = os.path.basename(BINDINGS)
    # Loudly rather than by skipping. An absent build removes the wasm target from `exports`,
    # after which every import of a renamed binding is counted, found to name no known
    # module, and reported. That is the right report for the wrong reason, and on a runner
    # that never built the bundle it would be the only thing this gate ever says.
    if not os.path.exists(BINDINGS):
        print(f'  {BINDINGS} is not on disk, so the one target whose names nobody writes by '
              f'hand goes unread. Run bash scripts/build-web.sh, then this.')
        return 1
    exports[bindings] = exported_names(open(BINDINGS).read()) | {'default'}

    unresolved, checked = 0, 0
    for module, entries in imports.items():
        for target, names in entries:
            if target not in exports:
                print(f'  no module named {target}, imported by {module}')
                unresolved += 1
                continue
            for imported in names:
                checked += 1
                if imported not in exports[target]:
                    print(f'  {module} imports {imported} from {target}, which does not export it')
                    unresolved += 1

    # Two counts that must not be zero, for two different reasons. Nothing checked at all
    # reads exactly like every import resolving. Nothing checked against the generated
    # bindings means the target this reads from disk was compared with nothing, which is the
    # shape a build present but empty would take.
    from_bindings = sum(len(names) for entries in imports.values()
                        for target, names in entries if target == bindings)
    if not checked:
        print(f'  {len(imports)} modules yielded no named import at all, so this compared nothing')
    if not from_bindings:
        print(f'  no module imports a name from {bindings}, so the generated bindings were '
              f'read from disk and compared with nothing')

    print(f'{checked} named imports across {len(imports)} modules, {from_bindings} of them '
          f'from {bindings}, {unresolved} unresolved')
    return 1 if unresolved or faults or not checked or not from_bindings else 0


if __name__ == '__main__':
    sys.exit(main())
