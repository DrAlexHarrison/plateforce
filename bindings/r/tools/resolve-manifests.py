"""Makes the copied engine manifests self-contained.

The engine crates inherit `{ workspace = true }` from the repository's root manifest. The
copy lives in a different workspace, so every inherited key is rewritten here to the value
the repository declares. Doing it to the copy rather than to a tracked file means the R
workspace never has to mirror a dependency list that the engine is free to change.

`rust-version` is the exception: the R crate's floor is its own, because it is the number
CRAN's oldest check machine is compared against.
"""

import re
import sys
import tomllib
from pathlib import Path

CRATES = ("plateforce-registry", "plateforce-core", "plateforce-analysis")


def spec_of(value):
    if isinstance(value, str):
        return f'"{value}"'
    parts = []
    for key, item in value.items():
        if key == "path":
            continue
        if isinstance(item, bool):
            parts.append(f"{key} = {str(item).lower()}")
        elif isinstance(item, list):
            inner = ", ".join(f'"{entry}"' for entry in item)
            parts.append(f"{key} = [{inner}]")
        else:
            parts.append(f'{key} = "{item}"')
    return "{ " + ", ".join(parts) + " }"


def main(repository, destination, floor):
    root = tomllib.loads((Path(repository) / "Cargo.toml").read_text())
    dependencies = root["workspace"]["dependencies"]
    package = dict(root["workspace"]["package"])
    package["rust-version"] = floor

    rewritten = 0
    for crate in CRATES:
        manifest = Path(destination) / crate / "Cargo.toml"
        text = manifest.read_text()

        def dependency(match):
            name = match.group(1)
            if name in ("plateforce-registry", "plateforce-core", "plateforce-analysis"):
                depth = "../" * 1
                return f'{name} = {{ path = "{depth}{name}" }}'
            if name not in dependencies:
                raise SystemExit(f"{crate} inherits {name}, which the root does not declare")
            return f"{name} = {spec_of(dependencies[name])}"

        text, count = re.subn(
            r"^([A-Za-z0-9_-]+) = \{ workspace = true \}$",
            dependency,
            text,
            flags=re.MULTILINE,
        )
        rewritten += count

        def field(match):
            name = match.group(1)
            if name not in package:
                raise SystemExit(f"{crate} inherits {name}, which the root does not declare")
            value = package[name]
            if isinstance(value, list):
                inner = ", ".join(f'"{entry}"' for entry in value)
                return f"{name} = [{inner}]"
            return f'{name} = "{value}"'

        text, count = re.subn(
            r"^([A-Za-z0-9_-]+)\.workspace = true$", field, text, flags=re.MULTILINE
        )
        rewritten += count
        manifest.write_text(text)

    print(f"{rewritten} inherited keys resolved across {len(CRATES)} crates")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3])
