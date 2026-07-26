"""Build the committed conformance fixture from a local corpus.

Writes one subject's force column and that subject's rows of the frozen reference
output into the conformance crate. Run it only when the fixture needs rebuilding; the
result is committed and the corpus is not.

Only one subject may be written, and only ever as a number. Source directory names are
athlete names, so nothing derived from a path component reaches an output file. The
script refuses any other subject rather than making that a matter of remembering.

    python3 audit/build_conformance_fixture.py <corpus-root> <reference-csv>
"""

from __future__ import annotations

import csv
import re
import sys
from pathlib import Path

PERMITTED_SUBJECT = 1
FORCE_COLUMN_INDEX = 2
FIXTURES = Path(__file__).resolve().parent.parent / "crates/plateforce-conformance/fixtures"


def force_column(source: Path) -> list[str]:
    """Take one column out of the export as text, without parsing it as a number."""
    values = []
    for line in source.read_text().splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        fields = stripped.split("\t")
        values.append(fields[FORCE_COLUMN_INDEX].strip())
    return values


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    corpus, reference = Path(argv[1]), Path(argv[2])
    FIXTURES.mkdir(parents=True, exist_ok=True)

    written = 0
    for path in sorted(corpus.rglob("AT*.txt")):
        matched = re.match(r"AT(\d+)_(\d+)\.txt$", path.name)
        if not matched:
            continue
        subject, trial = int(matched.group(1)), int(matched.group(2))
        if subject != PERMITTED_SUBJECT:
            continue
        values = force_column(path)
        target = FIXTURES / f"subject{subject:02d}_trial{trial}.force.txt"
        target.write_text("\n".join(values) + "\n")
        print(f"wrote {target.name}: {len(values)} samples")
        written += 1

    with reference.open() as handle:
        rows = list(csv.DictReader(handle))
    header = list(rows[0].keys())
    kept = [row for row in rows if int(row["subject"]) == PERMITTED_SUBJECT]
    target = FIXTURES / f"reference_subject{PERMITTED_SUBJECT:02d}.csv"
    with target.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=header)
        writer.writeheader()
        writer.writerows(kept)
    print(f"wrote {target.name}: {len(kept)} reference rows")

    if written != len(kept):
        print(
            f"trace count {written} does not match reference row count {len(kept)}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
