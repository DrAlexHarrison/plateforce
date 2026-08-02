#!/usr/bin/env python3
"""Write each walled entry's boundary into the registry, from the classification that made it.

What stops an entry being computed on a given recording is a fact about the entry, so it
belongs beside the entry rather than in a table somebody keeps in step by hand. The source
is the per-entry classification; this moves it, and never decides anything itself.

Idempotent: a second run over the same source changes nothing. The registry files are
parsed rather than windowed, because a fixed number of lines around an id crosses into the
next entry and the reader cannot tell.

The source is named on the command line rather than defaulted, because a path this program
cannot see is a thing to state rather than to guess at.
"""

import argparse
import pathlib
import sys
import tomllib

# The classification's words, and the field's. They differ on one, and the field's is what
# the surface that reports a boundary consumes.
BOUNDARY_OF_CLASS = {
    "PROTOCOL": "protocol",
    "EQUIPMENT": "equipment",
    "BOTH": "both",
    "SOURCE": "source",
    "UNCLASSIFIED": "undetermined",
}

# The order a run reports, so two runs read the same whatever the source's row order.
BOUNDARY_ORDER = ["protocol", "equipment", "both", "source", "undetermined"]


def classifications(path):
    """One row per walled entry: its id, its boundary, and the query that would settle it."""
    rows = []
    with open(path, encoding="utf-8") as handle:
        header = None
        for line in handle:
            if line.startswith("#") or not line.strip():
                continue
            fields = line.rstrip("\n").split("\t")
            if header is None:
                header = fields
                continue
            row = dict(zip(header, fields))
            named = row["class"]
            if named not in BOUNDARY_OF_CLASS:
                raise SystemExit(f"plateforce: {path} classifies {row['id']} as {named}")
            boundary = BOUNDARY_OF_CLASS[named]
            rows.append(
                {
                    "id": row["id"],
                    "boundary": boundary,
                    # Only the undetermined rows carry one. A query beside a settled boundary
                    # would read as doubt about a classification that was made.
                    "query": row.get("note", "").strip() if boundary == "undetermined" else "",
                }
            )
    return rows


def entries_in(registry):
    """Every method id the registry holds, with the file and the line its block opens on."""
    found = {}
    for path in sorted(pathlib.Path(registry, "methods").glob("*.toml")):
        text = path.read_text(encoding="utf-8")
        document = tomllib.loads(text)
        lines = text.splitlines(keepends=True)
        opens = [index for index, line in enumerate(lines) if line.strip() == "[[method]]"]
        if len(opens) != len(document.get("method", [])):
            raise SystemExit(
                f"plateforce: {path} parses {len(document.get('method', []))} entries and "
                f"{len(opens)} of its lines open one, so a block cannot be placed"
            )
        for ordinal, method in enumerate(document.get("method", [])):
            ends = opens[ordinal + 1] if ordinal + 1 < len(opens) else len(lines)
            found[method["id"]] = {
                "path": path,
                "opens": opens[ordinal],
                "ends": ends,
                "reach": method.get("reach"),
            }
    return found


def block_for(row):
    text = f'\n[method.reach]\nboundary = "{row["boundary"]}"\n'
    if row["query"]:
        escaped = row["query"].replace("\\", "\\\\").replace('"', '\\"')
        text += f'query = "{escaped}"\n'
    return text


def apply(registry, rows, write):
    placed = entries_in(registry)
    missing = [row["id"] for row in rows if row["id"] not in placed]
    if missing:
        raise SystemExit(
            f"plateforce: {len(missing)} of {len(rows)} classified ids are not in the "
            f"registry: {missing[:5]}"
        )

    wrong = []
    for row in rows:
        held = placed[row["id"]]["reach"]
        if held is None:
            wrong.append((row["id"], "no boundary", row["boundary"]))
        elif held.get("boundary") != row["boundary"]:
            wrong.append((row["id"], held.get("boundary"), row["boundary"]))

    if not write:
        return wrong

    # Written from the end of each file backwards, so an insertion never moves the line a
    # later block was measured at.
    by_file = {}
    for row in rows:
        if placed[row["id"]]["reach"] is None:
            by_file.setdefault(placed[row["id"]]["path"], []).append(row)
    for path, held in by_file.items():
        lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
        for row in sorted(held, key=lambda r: placed[r["id"]]["ends"], reverse=True):
            lines.insert(placed[row["id"]]["ends"], block_for(row))
        path.write_text("".join(lines), encoding="utf-8")
    return wrong


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--from", dest="source", required=True, help="the classification")
    parser.add_argument("--registry", default="registry", help="the registry directory")
    parser.add_argument("--write", action="store_true", help="write the blocks")
    parser.add_argument("--check", action="store_true", help="report without writing")
    arguments = parser.parse_args()

    if arguments.write == arguments.check:
        raise SystemExit("plateforce: name one of --write and --check")
    if not pathlib.Path(arguments.source).is_file():
        raise SystemExit(f"plateforce: {arguments.source} is not a file this program can read")

    rows = classifications(arguments.source)
    wrong = apply(arguments.registry, rows, arguments.write)

    counted = {name: 0 for name in BOUNDARY_ORDER}
    for row in rows:
        counted[row["boundary"]] += 1
    split = ", ".join(f"{counted[name]} {name}" for name in BOUNDARY_ORDER if counted[name])

    if arguments.check and wrong:
        for identifier, held, wanted in wrong:
            print(f"plateforce: {identifier} carries {held}, the classification says {wanted}",
                  file=sys.stderr)
        print(f"{len(rows) - len(wrong)} of {len(rows)} walled entries classified; {split}",
              file=sys.stderr)
        raise SystemExit(1)

    print(f"{len(rows)} of {len(rows)} walled entries classified; {split}")


if __name__ == "__main__":
    main()
