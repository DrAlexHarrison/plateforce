#!/usr/bin/env python3
"""The policy for a sample nobody measured has one home, and every reader reaches it.

What a reader does with a sample it cannot take as a measurement decides numbers, not
presentation. Holding it at the last real reading writes a force the plate never measured;
removing it closes the gap and shifts every timestamp after it. Three surfaces of this product
have each spelled that policy for themselves and each spelled it wrong, and the repair aimed at
the second was written in the same commit that left the third alone. So the check is not that
the current spellings agree. It is that there is one spelling.

A script over the sources rather than a test, because two of the five readers are not reachable
from a test in this workspace at all. R's binding crate carries its own workspace, and Python's
is a pyo3 module a Rust test cannot load. R is the surface where the policy was first found
wrong, so a check that could not see it would be a check that could not see the thing it exists
for. `crates/plateforce-wasm/tests/no_reader_repairs_the_recording_it_was_handed.rs` holds the
same property behaviourally, on the two readers a test can reach.

Two manifests, checked in both directions:

  `trial-readers.txt` names every reader. Each named function must reach the home and must
  build no trial of its own. A row whose function this cannot find is a refusal, because a
  renamed function matches nothing and matching nothing reads exactly like agreeing.

  `trial-constructions-outside-the-home.txt` names every other place shipped source builds a
  trial, and what out of. A site named nowhere is a refusal, so a sixth reader cannot appear
  quietly. A row naming a site that no longer builds one is a refusal too, so a repaired entry
  cannot sit here reading as a limitation somebody accepted.

Test sources are outside this, in files under a `tests/` directory and in the `#[cfg(test)]`
module of a source file. A test that builds a trial out of eight numbers is stating a fixture,
not deciding a policy.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).parent.parent
HOME = "plateforce-core/src/signal.rs"
HOME_FUNCTION = "trial_from_column"

READERS = pathlib.Path(__file__).with_name("trial-readers.txt")
OUTSIDE = pathlib.Path(__file__).with_name("trial-constructions-outside-the-home.txt")

# Where shipped Rust source lives. `bindings/r` is listed because it is not a member of this
# repository's workspace and nothing else in this file's reach would walk it.
SOURCE_ROOTS = ["crates", "bindings/r/src/rust/src"]

BUILDS_A_TRIAL = re.compile(r"\b(?:Core)?Trial::new\s*\(")
REACHES_THE_HOME = re.compile(rf"\b{HOME_FUNCTION}\s*\(")
DECLARES_A_FUNCTION = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)")


def rows_of(manifest):
    """One row per line, tab separated, comments and blank lines out."""
    rows = []
    for line in manifest.read_text(encoding="utf-8").splitlines():
        text = line.strip()
        if text and not text.startswith("#"):
            rows.append([cell.strip() for cell in text.split("\t")])
    if not rows:
        raise SystemExit(f"plateforce: {manifest.name} names nothing, so this check asserts nothing")
    return rows


def shipped_lines(path):
    """The file's lines up to its own test module, one-based, as (number, text).

    Cut at `#[cfg(test)]` rather than parsed, and the cut is stated rather than assumed: a
    source file that grew a second one would have everything after the first cut away, so the
    count of files this walked is printed and a file that went quiet shows up as a smaller
    number rather than as a pass.
    """
    lines = path.read_text(encoding="utf-8").splitlines()
    kept = []
    for number, text in enumerate(lines, start=1):
        if text.strip().startswith("#[cfg(test)]"):
            break
        kept.append((number, text))
    return kept


def enclosing_function(lines, at):
    """The nearest function declared at or above a line, by name.

    Nearest rather than outermost: a construction inside a closure inside a function belongs to
    the function, and a construction inside an `impl` block belongs to the method.
    """
    name = ""
    for number, text in lines:
        if number > at:
            break
        found = DECLARES_A_FUNCTION.match(text)
        if found:
            name = found.group(1)
    return name


def source_files():
    files = []
    for root in SOURCE_ROOTS:
        base = ROOT / root
        if not base.is_dir():
            raise SystemExit(f"plateforce: {root} is not a directory, so this check walked less than it says")
        for path in sorted(base.rglob("*.rs")):
            relative = path.relative_to(ROOT).as_posix()
            # A file under a `tests/` directory anywhere in its path, and the build artefacts
            # `cargo` and the R sync leave behind.
            if "/tests/" in f"/{relative}" or "/target/" in f"/{relative}":
                continue
            # The R package carries a copy of the engine, synced from `crates/` and untracked.
            # Walking it would check one source twice and report the copy's line numbers.
            if relative.startswith("bindings/r/src/rust/") and not relative.startswith(
                "bindings/r/src/rust/src/"
            ):
                continue
            files.append(path)
    if not files:
        raise SystemExit("plateforce: no source file was walked, so this check compared nothing")
    return files


def main():
    faults = []
    files = source_files()

    # Every construction of a trial in shipped source, by file and enclosing function.
    built_in = {}
    for path in files:
        relative = path.relative_to(ROOT).as_posix()
        lines = shipped_lines(path)
        for number, text in lines:
            if BUILDS_A_TRIAL.search(text):
                built_in.setdefault((relative, enclosing_function(lines, number)), []).append(number)

    print(f"walked {len(files)} shipped source files, {len(built_in)} of them build a trial")

    # The readers. Each reaches the home, and builds none of its own.
    reader_functions = set()
    for row in rows_of(READERS):
        if len(row) != 4:
            faults.append(f"{READERS.name}: a row needs a name, a file, a function and what it reads: {row}")
            continue
        name, relative, function, _reads = row
        path = ROOT / relative
        if not path.is_file():
            faults.append(f"{name} names {relative}, which is not a file")
            continue
        lines = shipped_lines(path)
        body = [text for number, text in lines if enclosing_function(lines, number) == function]
        if not body:
            faults.append(
                f"{name} names {relative}::{function}, which this check cannot find. A renamed "
                "function matches nothing, and matching nothing reads exactly like agreeing"
            )
            continue
        reader_functions.add((relative, function))
        joined = "\n".join(body)
        if not REACHES_THE_HOME.search(joined):
            faults.append(
                f"{name} reads a trace in {relative}::{function} and does not reach "
                f"{HOME_FUNCTION}, so it decides for itself what a sample nobody measured means"
            )
        if BUILDS_A_TRIAL.search(joined):
            faults.append(
                f"{name} builds a trial of its own in {relative}::{function}. Every reader that "
                "held or removed a sample did it by handing a rewritten vector to a constructor, "
                f"so a reader reaches {HOME_FUNCTION} and nothing else"
            )

    # Everything else that builds one, held to the register.
    declared = {}
    for row in rows_of(OUTSIDE):
        if len(row) != 3:
            faults.append(f"{OUTSIDE.name}: a row needs a file, a function and what it builds one out of: {row}")
            continue
        relative, function, _out_of = row
        declared[(relative, function)] = True

    for site, numbers in sorted(built_in.items()):
        relative, function = site
        if relative == f"crates/{HOME}" and function == HOME_FUNCTION:
            continue
        if site in reader_functions:
            continue  # already reported above, by name
        if site not in declared:
            faults.append(
                f"{relative}::{function} builds a trial at line(s) {numbers} and is named in "
                f"neither {READERS.name} nor {OUTSIDE.name}. A reader belongs in the first and "
                "anything else owes the second a sentence saying what the values are"
            )

    for site in sorted(declared):
        relative, function = site
        if site not in built_in:
            faults.append(
                f"{OUTSIDE.name} names {relative}::{function} as building a trial outside the "
                "home and it no longer builds one. An entry that outlived what it described "
                "reads as a limitation somebody accepted"
            )

    # The home builds one, or every assertion above is about a function that does nothing.
    if (f"crates/{HOME}", HOME_FUNCTION) not in built_in:
        faults.append(
            f"crates/{HOME}::{HOME_FUNCTION} builds no trial, so every reader above reaches a "
            "home that is not one"
        )

    if faults:
        print(f"\n{len(faults)} fault(s):", file=sys.stderr)
        for fault in faults:
            print(f"  {fault}", file=sys.stderr)
        return 1

    print(f"every reader reaches {HOME_FUNCTION}, and every other construction is named")
    return 0


if __name__ == "__main__":
    sys.exit(main())
