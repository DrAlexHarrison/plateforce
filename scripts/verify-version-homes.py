#!/usr/bin/env python3
"""Every place this project states its version, read and compared against each other.

A release names one thing. The number is written in four manifests and a tag is typed by
hand at the moment of release, and nothing until now compared the typed one to the written
ones. A tag `v0.2.0` cut against manifests reading 0.1.0 produces a release page titled
v0.2.0 carrying seven files named 0.1.0, and the artefact check passes, because the names
agree with each other.

    scripts/verify-version-homes.py                 the manifests agree
    scripts/verify-version-homes.py --expect 0.2.0  and they are this version
    scripts/verify-version-homes.py --ref-name v0.2.0 --ref-type tag

`--ref-type` that is not `tag` compares nothing and says so: a dispatch carries a branch
name, which names no version and must not be read as one.
"""

import argparse
import re
import sys
import tomllib
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parent.parent


def workspace_version(manifest):
    """The version a manifest states, whether it states it directly or inherits it."""
    parsed = tomllib.loads((REPOSITORY / manifest).read_text())
    package = parsed.get("package", {})
    stated = package.get("version")
    if isinstance(stated, str):
        return stated
    # `version.workspace = true` reads as {"workspace": True}, so the number lives in the
    # workspace table of the same file for a root manifest.
    return parsed["workspace"]["package"]["version"]


def description_version(path):
    """The `Version:` field of an R DESCRIPTION, which is not TOML and has no parser here."""
    for line in (REPOSITORY / path).read_text().splitlines():
        found = re.match(r"^Version:\s*(\S+)\s*$", line)
        if found:
            return found.group(1)
    raise SystemExit(f"{path} states no Version")


def homes():
    """Every file that states the version, and what each one says.

    Python is absent on purpose: `crates/plateforce-python/pyproject.toml` declares
    `dynamic = ["version"]` and maturin takes the number from the Rust manifest, so the
    wheel cannot disagree with the workspace and a fifth entry here would be the same
    reading twice.
    """
    return {
        "Cargo.toml": workspace_version("Cargo.toml"),
        "src-tauri/Cargo.toml": workspace_version("src-tauri/Cargo.toml"),
        "bindings/r/src/rust/Cargo.toml": workspace_version(
            "bindings/r/src/rust/Cargo.toml"
        ),
        "bindings/r/DESCRIPTION": description_version("bindings/r/DESCRIPTION"),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--expect")
    parser.add_argument("--ref-name")
    parser.add_argument("--ref-type")
    arguments = parser.parse_args()

    stated = homes()
    width = max(len(name) for name in stated)
    for name, version in stated.items():
        print(f"  {name:<{width}}  {version}")

    distinct = sorted(set(stated.values()))
    if len(distinct) > 1:
        print(
            f"the version homes disagree, so the artefacts would not name one release: "
            f"{', '.join(distinct)}",
            file=sys.stderr,
        )
        return 1
    agreed = distinct[0]
    print(f"version {agreed} in all {len(stated)} manifests")

    expected = arguments.expect
    if arguments.ref_name is not None:
        if arguments.ref_type != "tag":
            print(
                f"ref {arguments.ref_name!r} is a {arguments.ref_type}, which names no "
                f"version, so nothing was compared against it"
            )
            return 0
        # A leading `v` is the tag convention and is not part of the version. Anything else
        # is compared whole, so `release-1.0` fails here rather than being trimmed into
        # something that happens to match.
        expected = arguments.ref_name
        if expected.startswith("v"):
            expected = expected[1:]

    if expected is None:
        return 0

    if expected != agreed:
        print(
            f"the tag names {expected} and the manifests name {agreed}, so the release "
            f"page and the files inside it would carry different versions",
            file=sys.stderr,
        )
        return 1
    print(f"the tag and the manifests both name {agreed}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
