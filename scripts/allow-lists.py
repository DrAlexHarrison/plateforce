#!/usr/bin/env python3
"""Every list a gate consults to let something through, counted against the gates that hold them.

An allow-list that only ever grows is a coverage list that can never fail. Adding a name makes a
gate greener, and nothing goes red when the name stops being needed, so the list outlives the gap
it recorded and reads, to anybody counting green gates, exactly like coverage.

Two things separate a list worth keeping from a coverage hole wearing a whitelist, and this
reports both rather than either alone:

- **What the membership does.** A collection a gate iterates over drives the check. One a gate
  tests membership against in order to skip, negate or excuse is an allow-list. The name says
  nothing: `SUITES` drives and `KNOWN_TO_MISPARSE` excuses, and both are uppercase constants.
- **What it records.** A list holding a decision somebody made is one to keep. One holding
  "we have not got to this yet" is a coverage hole wearing a whitelist. Nothing mechanical
  tells those apart, so each is ruled by a reader and the ruling is what this checks.

Every candidate is ruled once, in `scripts/allow-lists-ruled.txt`. A population that drives a
check is `drives`. A permanent exclusion from that population is `exemption`. `excuses` is a
coverage hole that still lets a named case through. `--check` compares the set this finds against
the set that file records. It fails in both directions: a list nobody has ruled on is red, and a
ruling the census no longer finds is red.

What this does not do is decide whether a list is pinned. A verdict that is true and whose reason
is false is worse than none, so pinning is ruled by a reader in the file and never guessed here.

Run: python3 scripts/allow-lists.py [--json] [--check]
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RULINGS = ROOT / "scripts" / "allow-lists-ruled.txt"

SKIP_DIRECTORIES = {"target", "node_modules", ".git", "pkg", "dist", ".venv"}
# This file's own tables are membership tests about membership tests. Walking itself would count
# them, and a census that counts its own instrument reports one more than there is.
SKIP_FILES = {"scripts/allow-lists.py"}


def gate_sources() -> dict[str, list[Path]]:
    """The corpus: everything CI runs to decide green, by the language it is written in.

    Written out as a corpus rather than left to one glob so the denominator every count below
    carries is a list a reader can check.
    """
    corpus: dict[str, list[Path]] = {"rust": [], "python": [], "javascript": [], "r": []}
    for path in sorted(ROOT.rglob("*")):
        if not path.is_file() or set(path.parts) & SKIP_DIRECTORIES:
            continue
        relative = path.relative_to(ROOT)
        if str(relative) in SKIP_FILES:
            continue
        parts = set(relative.parts)
        if path.suffix == ".rs":
            body = path.read_text(errors="replace")
            if "tests" in parts or "#[cfg(test)]" in body or relative.name == "build.rs":
                corpus["rust"].append(relative)
        elif path.suffix in {".py", ".mjs"} and relative.parts[0] == "scripts":
            corpus["python" if path.suffix == ".py" else "javascript"].append(relative)
        elif path.suffix == ".R" and ("tests" in parts or relative.parts[:2] == ("bindings", "r")):
            corpus["r"].append(relative)
    return corpus


# Where a gate asks whether a name is in a collection. The collection is the capture.
MEMBERSHIP = [
    re.compile(r"(?P<negated>!)?\s*(?P<name>[A-Z][A-Z0-9_]{2,})\s*\.\s*contains\s*\("),
    re.compile(r"(?P<negated>!)?\s*(?P<name>[A-Z][A-Z0-9_]{2,})\s*\.\s*(?:includes|has)\s*\("),
    re.compile(r"(?P<negated>not\s+)?in\s+(?P<name>[A-Z][A-Z0-9_]{2,})\b"),
    re.compile(r"%in%\s*(?P<name>[A-Z][A-Z0-9_.]{2,})\b"),
]

# What makes a membership test an exemption rather than a check: the answer skips the work, or
# the test is negated so only the names outside the list are held to anything.
EXEMPTING_IN_THE_SAME_STATEMENT = re.compile(
    r"\b(continue|skip|next|return|pass|retain|filter|remove|discard|exclude|drop)\b|"
    r"\bnot\s+in\b|![A-Z]"
)

DECLARATIONS = [
    re.compile(r"(?:pub(?:\([a-z]+\))?\s+)?(?:const|static)\s+(?P<name>[A-Z][A-Z0-9_]*)\s*:[^=\n]+=\s*&?(?P<open>[\[\{])"),
    re.compile(r"^(?P<name>[A-Z][A-Z0-9_]*)\s*(?::[^=\n]+)?=\s*(?:frozenset\(|set\()?(?P<open>[\[\{\(])", re.M),
    re.compile(r"const\s+(?P<name>[A-Z][A-Za-z0-9_]*)\s*=\s*(?:new\s+Set\()?(?P<open>[\[\{])"),
    re.compile(r"^(?P<name>[A-Z][A-Z0-9_.]*)\s*<-\s*c(?P<open>\()", re.M),
]

CLOSING = {"[": "]", "{": "}", "(": ")"}


def literal_at(text: str, index: int, opener: str) -> str:
    depth = 0
    for position in range(index, len(text)):
        if text[position] == opener:
            depth += 1
        elif text[position] == CLOSING[opener]:
            depth -= 1
            if depth == 0:
                return text[index : position + 1]
    return text[index : index + 600]


def declaration_of(text: str, name: str) -> tuple[int, str] | None:
    """Where the collection is declared and what it holds, or nothing when it is declared
    elsewhere. A list consulted in one file and declared in another is reported by the file
    that declares it, so nothing is counted twice."""
    for pattern in DECLARATIONS:
        for match in pattern.finditer(text):
            if match.group("name") == name:
                return match.start(), literal_at(text, match.start("open"), match.group("open"))
    return None


def statement_around(text: str, index: int) -> str:
    """The membership test with enough of its statement to see what the answer does with it.

    A window rather than a language parser. A braced conditional runs through its matching
    close; every other statement runs through the following line.
    """
    start = text.rfind("\n", 0, text.rfind("\n", 0, index) + 1) + 1
    current_line = text.rfind("\n", 0, index) + 1
    line_end = text.find("\n", index)
    brace = text.find("{", index, line_end if line_end != -1 else len(text))
    if brace != -1 and re.search(r"\bif\b", text[current_line:brace]):
        depth = 0
        for position in range(brace, len(text)):
            if text[position] == "{":
                depth += 1
            elif text[position] == "}":
                depth -= 1
                if depth == 0:
                    return text[start : position + 1]
    end = text.find("\n", (line_end if line_end != -1 else index) + 1)
    return text[start : end if end != -1 else len(text)]


def audit() -> dict:
    corpus = gate_sources()
    lists: dict[tuple[str, str], dict] = {}
    driving = 0
    for language, paths in corpus.items():
        for relative in paths:
            text = (ROOT / relative).read_text(errors="replace")
            seen: dict[str, list[str]] = {}
            for pattern in MEMBERSHIP:
                for match in pattern.finditer(text):
                    seen.setdefault(match.group("name"), []).append(
                        statement_around(text, match.start())
                    )
            for name, statements in seen.items():
                declared = declaration_of(text, name)
                if declared is None:
                    continue
                position, body = declared
                entries = len(re.findall(r'"[^"]*"|\'[^\']*\'', body))
                if entries == 0:
                    # An empty population is the closed shape rather than an absent one. The
                    # register of unasked surfaces is keyed by request kind, so its empty
                    # nested set is the population the membership test reads.
                    empty_population = re.search(r"^[\[\{]\s*[\]\}]$", body, re.S) or re.search(
                        r":\s*\{\s*\}", body
                    )
                    if not empty_population:
                        continue
                exempting = [
                    statement
                    for statement in statements
                    if EXEMPTING_IN_THE_SAME_STATEMENT.search(statement)
                ]
                if not exempting:
                    driving += 1
                    continue
                lists[(str(relative), name)] = {
                    "language": language,
                    "file": str(relative),
                    "name": name,
                    "entries": entries,
                    "line": text[:position].count("\n") + 1,
                    "exempting_use": exempting[0].strip()[:160],
                }
    rows = sorted(lists.values(), key=lambda row: (row["file"], row["line"]))
    return {
        "corpus": {language: len(paths) for language, paths in corpus.items()},
        "corpus_total": sum(len(paths) for paths in corpus.values()),
        "driving_collections": driving,
        "allow_lists": rows,
    }


def ruled() -> dict[str, tuple[str, str]]:
    """The rulings, keyed by `<file>:<NAME>`, as the verdict and the reasoning under it."""
    rulings: dict[str, tuple[str, str]] = {}
    key = None
    for line in RULINGS.read_text().splitlines():
        if line.startswith("#") or not line.strip():
            continue
        if not line.startswith(" "):
            verdict, key = line.split(None, 1)
            rulings[key.strip()] = (verdict, "")
        elif key is not None:
            verdict, reasoning = rulings[key.strip()]
            rulings[key.strip()] = (verdict, (reasoning + " " + line.strip()).strip())
    return rulings


def main() -> int:
    report = audit()
    if "--json" in sys.argv:
        print(json.dumps(report, indent=2))
        return 0

    rows = report["allow_lists"]
    rulings = ruled()
    excusing = [
        row for row in rows if rulings.get(f"{row['file']}:{row['name']}", ("", ""))[0] == "excuses"
    ]
    exemptions = [
        row
        for row in rows
        if rulings.get(f"{row['file']}:{row['name']}", ("", ""))[0] == "exemption"
    ]
    print(
        f"{report['corpus_total']} gate sources walked: "
        + ", ".join(f"{count} {language}" for language, count in report["corpus"].items())
    )
    print(
        f"{len(rows)} of them hold a list consulted to skip, negate or excuse: "
        f"{len(exemptions)} genuine exemptions and {len(excusing)} coverage holes"
    )
    print(
        f"{report['driving_collections']} further collections are tested for membership and "
        f"drive their check rather than excusing anything"
    )
    print()
    width = max((len(row["name"]) for row in rows), default=4)
    for row in rows:
        verdict = rulings.get(f"{row['file']}:{row['name']}", ("UNRULED", ""))[0]
        print(
            f"  {row['name']:<{width}}  {row['entries']:>3} entries  {verdict:<8}  "
            f"{row['file']}:{row['line']}"
        )

    if "--check" not in sys.argv:
        return 0

    found = {f"{row['file']}:{row['name']}" for row in rows}
    recorded = set(rulings)
    allowed_verdicts = {"drives", "exemption", "excuses"}
    invalid = sorted(key for key, (verdict, _) in rulings.items() if verdict not in allowed_verdicts)
    unreasoned = sorted(key for key, (_, reasoning) in rulings.items() if not reasoning)
    print()
    # An equality rather than a floor, so this fails in both directions. A list nobody has ruled
    # on is a check that cannot fail for whatever it names, and a ruling the census no longer
    # finds is a ruling that outlived its list. A file compared by `issubset` would pass on the
    # second, which is the shape this file exists to refuse.
    if found == recorded and not invalid and not unreasoned:
        print(f"{len(found)} lists found and {len(recorded)} ruled, and they are the same set.")
        return 0
    for key in sorted(found - recorded):
        print(f"  not ruled on: {key}")
    for key in sorted(recorded - found):
        print(f"  ruled and no longer found: {key}")
    for key in invalid:
        print(f"  unknown verdict: {key} says {rulings[key][0]}")
    for key in unreasoned:
        print(f"  ruling gives no reason: {key}")
    print(
        f"{len(found)} found against {len(recorded)} ruled. Read the list, rule on it in "
        f"{RULINGS.relative_to(ROOT)}, and say what it records."
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
