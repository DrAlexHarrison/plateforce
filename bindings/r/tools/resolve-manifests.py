"""Makes the copied engine manifests self-contained.

The engine crates inherit `{ workspace = true }` from the repository's root manifest. The
copy lives in a different workspace, so every inherited key is rewritten here to the value
the repository declares. Doing it to the copy rather than to a tracked file means the R
workspace never has to mirror a dependency list that the engine is free to change.

`rust-version` is the exception: the R crate's floor is its own, because it is the number
CRAN's oldest check machine is compared against.

The crate a manifest names and the folder its copy sits in are not the same word, because
the folder is shortened to keep every path inside what a tarball is required to store.
`tools/sync-engine.sh` owns that mapping and passes it here as `crate:folder` arguments, so
the path dependency each copy declares on its siblings points at where they actually landed.
"""

import re
import sys
import tomllib
from pathlib import Path


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


def main(repository, destination, floor, pairs):
    # An empty mapping would leave every sibling dependency to fall through to the root's
    # own table, whose spec carries a version and no path, and the copy would then resolve
    # against a crates.io that has never heard of these crates. Refusing here names the
    # cause; letting it through would surface as a resolution error three steps later.
    if not pairs:
        raise SystemExit("no crate:folder pairs given, so no manifest would be resolved")
    folders = dict(pair.split(":", 1) for pair in pairs)

    root = tomllib.loads((Path(repository) / "Cargo.toml").read_text())
    dependencies = root["workspace"]["dependencies"]
    package = dict(root["workspace"]["package"])
    package["rust-version"] = floor

    rewritten = 0
    for crate, folder in folders.items():
        manifest = Path(destination) / folder / "Cargo.toml"
        text = manifest.read_text()

        def dependency(match):
            name = match.group(1)
            if name in folders:
                return f'{name} = {{ path = "../{folders[name]}" }}'
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

        # A substitution that matches nothing writes the file back unchanged and reports
        # success, and the copy then resolves only for as long as it happens to sit beside
        # the workspace it inherited from. Whatever is left saying so is named here. The
        # search is for the words anywhere on a line rather than for the two shapes the
        # patterns above expect, because a third shape neither of them covers,
        # `serde = { workspace = true, features = ["derive"] }`, is precisely the one that
        # would pass through both of them unread.
        unresolved = re.search(r"^.*workspace\s*=\s*true.*$", text, flags=re.MULTILINE)
        if unresolved:
            raise SystemExit(f"{crate} still inherits: {unresolved.group(0).strip()}")

        manifest.write_text(text)

    print(f"{rewritten} inherited keys resolved across {len(folders)} crates")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:])
