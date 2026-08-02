#!/usr/bin/env python3
"""Which workstream changed which registry entry, and whether it was allowed to.

Eight agents share one working tree and ten of the fifteen characterised method files
hold entries belonging to two of them, so the unit of ownership is the entry rather
than the file.

Entries are parsed and compared as data. A line window around an id runs into the next
entry and reports a change that is not there, which is the shape of query this project
has already been wrong with.
"""

import argparse
import glob
import json
import os
import subprocess
import sys
import tomllib

ENTRY_ARRAYS = ("method", "protocol", "construct")
REGISTRY_GLOBS = ("registry/*.toml", "registry/**/*.toml")


def repository_root():
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def read_manifest(path):
    with open(path, "rb") as handle:
        manifest = tomllib.load(handle)
    by_id = {}
    for block in manifest.get("entry", []):
        workstream = block["workstream"]
        for entry_id in block["ids"]:
            if entry_id in by_id and by_id[entry_id] != workstream:
                raise SystemExit(
                    f"manifest names {entry_id} for both {by_id[entry_id]} and {workstream}"
                )
            by_id[entry_id] = workstream
    return manifest.get("file_default", {}), by_id


def owner_of(path, entry_id, file_defaults, by_id):
    """Most specific first: a named id, then the file, then its directory."""
    if entry_id in by_id:
        return by_id[entry_id], "named"
    if path in file_defaults:
        return file_defaults[path], "file"
    directory = os.path.dirname(path)
    while directory:
        if directory in file_defaults:
            return file_defaults[directory], "directory"
        directory = os.path.dirname(directory)
    return None, "unowned"


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, default=repr)


def parse_entries(text, path):
    """Serialised form and population per id, plus everything that is not an entry.

    Two entries sharing an id would collapse into one key and hide every change to the
    shadowed one, so a duplicate is an error rather than a last-one-wins.
    """
    document = tomllib.loads(text)
    entries = {}
    for array in ENTRY_ARRAYS:
        for position, entry in enumerate(document.get(array, []), 1):
            entry_id = entry.get("id")
            if entry_id is None:
                raise SystemExit(f"{path}: [[{array}]] #{position} carries no id")
            if entry_id in entries:
                raise SystemExit(f"{path}: id {entry_id} appears more than once")
            entries[entry_id] = (array, canonical(entry))
    outside = {k: v for k, v in document.items() if k not in ENTRY_ARRAYS}
    return entries, canonical(outside)


def registry_files_in_tree(root):
    found = set()
    for pattern in REGISTRY_GLOBS:
        for path in glob.glob(os.path.join(root, pattern), recursive=True):
            found.add(os.path.relpath(path, root))
    return found


def registry_files_in_ref(root, ref):
    result = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", ref, "registry/"],
        capture_output=True,
        text=True,
        cwd=root,
    )
    if result.returncode != 0:
        raise SystemExit(f"cannot list {ref}: {result.stderr.strip()}")
    return {p for p in result.stdout.splitlines() if p.endswith(".toml")}


def text_in_ref(root, ref, path):
    result = subprocess.run(
        ["git", "show", f"{ref}:{path}"], capture_output=True, text=True, cwd=root
    )
    return result.stdout if result.returncode == 0 else None


def text_in_tree(root, path):
    absolute = os.path.join(root, path)
    if not os.path.exists(absolute):
        return None
    with open(absolute, encoding="utf-8") as handle:
        return handle.read()


def changes(root, ref):
    """Every id whose data differs between the ref and the working tree."""
    paths = sorted(registry_files_in_ref(root, ref) | registry_files_in_tree(root))
    found = []
    for path in paths:
        before_text = text_in_ref(root, ref, path)
        after_text = text_in_tree(root, path)
        before, before_outside = (
            parse_entries(before_text, path) if before_text is not None else ({}, "{}")
        )
        after, after_outside = (
            parse_entries(after_text, path) if after_text is not None else ({}, "{}")
        )
        for entry_id in sorted(set(before) | set(after)):
            if entry_id not in after:
                found.append((path, entry_id, "removed"))
            elif entry_id not in before:
                found.append((path, entry_id, "added"))
            elif before[entry_id][1] != after[entry_id][1]:
                found.append((path, entry_id, "changed"))
        if before_outside != after_outside:
            found.append((path, None, "file content outside any entry"))
    return found


def check(root, ref, workstream, file_defaults, by_id):
    found = changes(root, ref)
    if not found:
        print(f"no registry entry changed against {ref}")
        return 0
    violations = []
    for path, entry_id, kind in found:
        if entry_id is None:
            owner, rule = owner_of(path, "", file_defaults, by_id)
        else:
            owner, rule = owner_of(path, entry_id, file_defaults, by_id)
        subject = entry_id if entry_id else path
        if owner is None:
            violations.append(f"  {subject} ({kind}, {path}) is owned by no workstream")
        elif owner != workstream:
            violations.append(
                f"  {subject} ({kind}, {path}) belongs to {owner}, resolved by {rule}"
            )
        else:
            print(f"  {subject} ({kind}) is {workstream}'s")
    if violations:
        print(f"\n{len(violations)} of {len(found)} changes are outside {workstream}:")
        for line in violations:
            print(line)
        return 1
    print(f"\nall {len(found)} changes are {workstream}'s")
    return 0


def audit(root, file_defaults, by_id):
    """Every id in the tree with its owner, so a hole in the manifest is visible.

    Counted per population and reported against its own denominator. The registry's
    populations are separate quantities and adding them produces a number that
    describes nothing.
    """
    counts = {array: {} for array in ENTRY_ARRAYS}
    totals = {array: 0 for array in ENTRY_ARRAYS}
    unowned = {array: [] for array in ENTRY_ARRAYS}
    for path in sorted(registry_files_in_tree(root)):
        text = text_in_tree(root, path)
        entries, _ = parse_entries(text, path)
        for entry_id, (array, _serialised) in entries.items():
            totals[array] += 1
            owner, _ = owner_of(path, entry_id, file_defaults, by_id)
            if owner is None:
                unowned[array].append(f"{entry_id} ({path})")
            else:
                counts[array][owner] = counts[array].get(owner, 0) + 1
    status = 0
    for array in ENTRY_ARRAYS:
        if not totals[array]:
            continue
        print(f"{array} entries, {totals[array]}")
        for owner in sorted(counts[array]):
            print(f"  {counts[array][owner]:4d} of {totals[array]}  {owner}")
        if unowned[array]:
            print(f"  {len(unowned[array])} of {totals[array]} owned by no workstream:")
            for line in unowned[array]:
                print(f"    {line}")
            status = 1
        print()
    return status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workstream", help="the workstream making the change")
    parser.add_argument("--base", default="HEAD", help="the ref to compare against")
    parser.add_argument(
        "--audit", action="store_true", help="print every entry with its owner"
    )
    arguments = parser.parse_args()

    root = repository_root()
    file_defaults, by_id = read_manifest(
        os.path.join(root, "scripts", "entry-ownership.toml")
    )

    if arguments.audit:
        return audit(root, file_defaults, by_id)
    if not arguments.workstream:
        parser.error("--workstream is required unless --audit is given")
    return check(root, arguments.base, arguments.workstream, file_defaults, by_id)


if __name__ == "__main__":
    sys.exit(main())
