#!/usr/bin/env python3
"""Checks that the committed tree is the tree, not the one on somebody's disk.

    python3 scripts/verify-committed-tree.py            HEAD
    python3 scripts/verify-committed-tree.py origin/main
    python3 scripts/verify-committed-tree.py HEAD --compile

A module declared but not committed compiles for everyone who has the file sitting
untracked beside them, and fails for anyone who clones. Eight agents sharing one checkout
all have the file; CI and a new contributor do not. It has happened twice here.

The default check is static and takes under a second, so it can run before a push rather
than after one. `--compile` extracts the ref and builds it, which catches everything the
static check cannot but costs a cold build.
"""

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path, PurePosixPath

# A declaration ending in a semicolon needs a file. One ending in a brace carries its own body.
DECLARES_A_MODULE = re.compile(
    r"^\s*(?:#\[path\s*=\s*\"(?P<path>[^\"]+)\"\]\s*)?"
    r"(?:pub\s*(?:\([^)]*\)\s*)?)?mod\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*;",
    re.M,
)
COMMENTS = re.compile(r"//[^\n]*|/\*.*?\*/", re.S)

# `include!` splices a file's tokens in, so the file is compiled while no `mod` names it.
# `include_str!` and `include_bytes!` embed rather than compile, and are followed too: the
# question worth asking is whether anything references the file, because that is what makes
# the remedy safe. Telling somebody to delete a file another file embeds breaks the build.
# The path resolves against the directory of the file doing the including, not against where
# that file's child modules would live.
INCLUDES_A_FILE = re.compile(r"\binclude(?:_str|_bytes)?!\s*\(\s*\"(?P<path>[^\"]+)\"")
OPENS_A_MACRO_BODY = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*!\s*[{(\[]")

# A file named for the module it opens holds its children beside it; any other file holds
# them in a directory named after itself.
OPENS_ITS_DIRECTORY = {"lib.rs", "main.rs", "mod.rs", "build.rs"}

# Cargo discovers a target per file directly inside these, so each such file is its own crate
# root and its children sit beside it rather than under it.
DIRECTORIES_OF_CRATE_ROOTS = {"tests", "benches", "examples"}


def without_macro_bodies(source: str) -> str:
    """Drops what macros are handed, because a DSL is free to spell anything like Rust.

    extendr's `extendr_module! { mod plateforce; }` reads exactly like a declaration that
    needs a file on disk, and needs none.
    """
    kept = []
    at = 0
    while True:
        opening = OPENS_A_MACRO_BODY.search(source, at)
        if not opening:
            kept.append(source[at:])
            return "".join(kept)
        kept.append(source[at : opening.end() - 1])
        closing = {"{": "}", "(": ")", "[": "]"}[source[opening.end() - 1]]
        depth = 0
        cursor = opening.end() - 1
        while cursor < len(source):
            if source[cursor] == source[opening.end() - 1]:
                depth += 1
            elif source[cursor] == closing:
                depth -= 1
                if depth == 0:
                    break
            cursor += 1
        at = cursor + 1


def committed_paths(ref: str) -> set[str]:
    listing = subprocess.run(
        ["git", "ls-tree", "-r", ref, "--name-only"],
        check=True, capture_output=True, text=True,
    ).stdout
    return set(listing.splitlines())


def contents(ref: str, path: str) -> str:
    return subprocess.run(
        ["git", "show", f"{ref}:{path}"],
        check=True, capture_output=True, text=True,
    ).stdout


def where_children_live(source: PurePosixPath) -> PurePosixPath:
    if source.name in OPENS_ITS_DIRECTORY:
        return source.parent
    if source.parent.name in DIRECTORIES_OF_CRATE_ROOTS:
        return source.parent
    return source.parent / source.stem


def unresolved_modules(ref: str) -> list[str]:
    paths = committed_paths(ref)
    missing = []
    for path in sorted(p for p in paths if p.endswith(".rs")):
        source = PurePosixPath(path)
        directory = where_children_live(source)
        readable = without_macro_bodies(COMMENTS.sub("", contents(ref, path)))
        for declaration in DECLARES_A_MODULE.finditer(readable):
            named = declaration.group("path")
            if named:
                candidates = [str(directory / named)]
            else:
                child = declaration.group("name")
                candidates = [str(directory / f"{child}.rs"), str(directory / child / "mod.rs")]
            if not any(candidate in paths for candidate in candidates):
                missing.append(f"{path} declares {declaration.group('name')}, and none of "
                               f"{', '.join(candidates)} is committed")
    return missing


def names_of(ref: str, path: str) -> set[str]:
    """The paths this one file's declarations name."""
    directory = where_children_live(PurePosixPath(path))
    uncommented = COMMENTS.sub("", contents(ref, path))
    readable = without_macro_bodies(uncommented)
    named: set[str] = set()
    for declaration in DECLARES_A_MODULE.finditer(readable):
        given = declaration.group("path")
        if given:
            named.add(str(directory / given))
        else:
            child = declaration.group("name")
            named.add(str(directory / f"{child}.rs"))
            named.add(str(directory / child / "mod.rs"))
    # An included file is compiled without any `mod` naming it, so a walk that follows only
    # declarations reports it as unreachable and then tells a reader to declare it, which
    # would define it twice, or delete it, which would delete compiled code.
    for inclusion in INCLUDES_A_FILE.finditer(uncommented):
        here = PurePosixPath(path).parent
        named.add(str(PurePosixPath(os.path.normpath(str(here / inclusion.group("path"))))))
    return named


def crate_roots(paths: set[str]) -> set[str]:
    """Every path cargo compiles without anything naming it."""
    roots: set[str] = set()
    for path in (p for p in paths if p.endswith(".rs")):
        source = PurePosixPath(path)
        if source.name in {"lib.rs", "main.rs", "build.rs"}:
            roots.add(path)
        elif source.parent.name in DIRECTORIES_OF_CRATE_ROOTS or source.parent.name == "bin":
            roots.add(path)
    return roots


def reachable_from_roots(ref: str, paths: set[str]) -> set[str]:
    """Walk out from the crate roots, so an orphan cannot vouch for its own children.

    A single pass collecting every path any file names says only "somebody names this",
    which an unreachable file satisfies for the modules beneath it. Those two questions
    agree exactly while an orphan is one file, and `preset.rs` was one file, so the first
    version of this check answered the easier one and looked correct.
    """
    reached = crate_roots(paths)
    frontier = list(reached)
    while frontier:
        path = frontier.pop()
        for candidate in names_of(ref, path):
            if candidate in paths and candidate not in reached:
                reached.add(candidate)
                frontier.append(candidate)
    return reached


def unreachable_modules(ref: str) -> list[str]:
    """Files that compile for nobody, because no crate root reaches them.

    The mirror of `unresolved_modules`, and the quieter direction of the same fault. A
    declaration without its file fails a clean build loudly with E0583. A file without its
    declaration fails nothing at all: it is never compiled, never linted and never tested,
    and it reads as shipped code to anyone who opens it. `preset.rs` sat in
    `plateforce-registry` that way, and declaring it cost five compile errors that no
    reviewer had seen.
    """
    paths = committed_paths(ref)
    reached = reachable_from_roots(ref, paths)
    orphans = []
    for path in sorted(p for p in paths if p.endswith(".rs")):
        if path in reached:
            continue
        orphans.append(f"{path} is committed and no crate root reaches it, so it is never compiled")
    return orphans


def compiles(ref: str) -> bool:
    with tempfile.TemporaryDirectory() as scratch:
        archive = subprocess.Popen(["git", "archive", ref], stdout=subprocess.PIPE)
        subprocess.run(["tar", "-x", "-C", scratch], stdin=archive.stdout, check=True)
        archive.wait()
        built = subprocess.run(
            ["cargo", "check", "--workspace", "--locked", "--all-targets"],
            cwd=scratch,
        )
        return built.returncode == 0


def main() -> int:
    arguments = [word for word in sys.argv[1:] if not word.startswith("--")]
    ref = arguments[0] if arguments else "HEAD"
    also_compile = "--compile" in sys.argv[1:]

    missing = unresolved_modules(ref)
    if missing:
        print(f"{ref} does not build from a clean checkout:", file=sys.stderr)
        for line in missing:
            print(f"  {line}", file=sys.stderr)
        print("\ncommit the file, or drop the declaration until it exists", file=sys.stderr)
        return 1

    print(f"{ref}: every declared module is committed")

    orphans = unreachable_modules(ref)
    if orphans:
        print(f"\n{ref} carries source no crate root reaches:", file=sys.stderr)
        for line in orphans:
            print(f"  {line}", file=sys.stderr)
        print("\ndeclare it beside its module, or delete it until it is wanted", file=sys.stderr)
        return 1

    print(f"{ref}: every committed module is reachable")

    if also_compile and not compiles(ref):
        print(f"{ref} does not compile from a clean checkout", file=sys.stderr)
        return 1
    if also_compile:
        print(f"{ref}: compiles from a clean checkout")
    return 0


if __name__ == "__main__":
    sys.exit(main())
