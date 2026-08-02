#!/usr/bin/env python3
"""Every name one browser module imports is a name the module it names exports.

A split that leaves an import dangling loads as a blank page with one console line, so this
runs as a gate rather than as a screen check. The generated wasm bindings are read from
disk like any other target, so a renamed export fails here rather than at runtime.
"""

import glob
import os
import re
import sys

IDENTIFIER = r'[\w$]+'


def exported_names(source: str) -> set:
    names = set(re.findall(rf'^export\s+(?:async\s+)?function\s+({IDENTIFIER})', source, re.M))
    names |= set(re.findall(rf'^export\s+(?:const|let|var|class)\s+({IDENTIFIER})', source, re.M))
    for block in re.findall(r'^export\s*\{([^}]*)\}', source, re.M):
        names |= {t.strip().split(' as ')[-1].strip() for t in block.split(',') if t.strip()}
    return names


def main() -> int:
    exports, imports = {}, {}
    for path in sorted(glob.glob('web/*.js')):
        source = open(path).read()
        name = os.path.basename(path)
        exports[name] = exported_names(source)
        imports[name] = [
            (os.path.basename(target),
             [t.strip().split(' as ')[0].strip() for t in block.split(',') if t.strip()])
            for block, target in re.findall(
                rf"import\s+(?:{IDENTIFIER}\s*,\s*)?\{{([^}}]*)\}}\s*from\s*'([^']+)'", source)
        ]

    bindings = 'web/pkg/plateforce_wasm.js'
    if os.path.exists(bindings):
        source = open(bindings).read()
        exports[os.path.basename(bindings)] = exported_names(source) | {'default'}

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

    print(f'{checked} named imports across {len(imports)} modules, {unresolved} unresolved')
    return 1 if unresolved else 0


if __name__ == '__main__':
    sys.exit(main())
