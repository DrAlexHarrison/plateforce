#!/usr/bin/env python3
"""Renders the desktop icon set from the mark the product already ships.

The mark has one home, the header of `web/index.html`, and its two colours have one home,
the token block at the top of `web/styles.css`. Reading both here rather than storing a
second copy of the artwork is what stops the icon on somebody's dock drifting away from
the mark on the page. A mark this cannot find is an error rather than a fallback drawing.

    python3 scripts/render-icons.py

Writes `src-tauri/icons/`, including the `Square*Logo.png` and `StoreLogo.png` files the
Windows Store package reads, which are not spare.
"""

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parent.parent
RENDERED_EDGE_PIXELS = 1024


def the_mark_from(index_html: str) -> tuple[str, str]:
    """The viewBox and the inner shapes of the header mark, verbatim."""
    block = re.search(
        r'<svg class="app-header__mark"[^>]*viewBox="([^"]+)"[^>]*>(.*?)</svg>',
        index_html,
        re.DOTALL,
    )
    if block is None:
        raise SystemExit(
            "web/index.html carries no svg with class app-header__mark; "
            "the icon is rendered from that mark and there is nothing else to draw"
        )
    return block.group(1), block.group(2).strip()


def the_colour(styles_css: str, token: str) -> str:
    """The first value a token is given, which is the light one the shipped favicon uses."""
    found = re.search(rf"^\s*{re.escape(token)}:\s*(#[0-9a-fA-F]{{3,8}})\s*;", styles_css, re.M)
    if found is None:
        raise SystemExit(f"web/styles.css declares no {token}, which the mark is drawn in")
    return found.group(1)


def render_to_png(svg: str, destination: Path) -> None:
    with tempfile.NamedTemporaryFile("w", suffix=".svg", delete=False) as handle:
        handle.write(svg)
        source = Path(handle.name)
    try:
        if shutil.which("rsvg-convert"):
            command = [
                "rsvg-convert", "-w", str(RENDERED_EDGE_PIXELS), "-h",
                str(RENDERED_EDGE_PIXELS), str(source), "-o", str(destination),
            ]
        elif shutil.which("inkscape"):
            command = [
                "inkscape", str(source), f"--export-filename={destination}",
                f"--export-width={RENDERED_EDGE_PIXELS}",
                f"--export-height={RENDERED_EDGE_PIXELS}",
            ]
        else:
            raise SystemExit("neither rsvg-convert nor inkscape is here to render the mark")
        subprocess.run(command, check=True, capture_output=True)
    finally:
        source.unlink(missing_ok=True)


def main() -> None:
    view_box, shapes = the_mark_from((REPOSITORY / "web/index.html").read_text())
    styles = (REPOSITORY / "web/styles.css").read_text()
    # The stylesheet paints the mark through two tokens and the shipped favicon hardcodes
    # what they resolve to, so the icon on a dock is the mark in a tab.
    shapes = shapes.replace("<rect ", f'<rect fill="{the_colour(styles, "--accent")}" ')
    shapes = shapes.replace(
        "<path ", f'<path stroke="{the_colour(styles, "--accent-contrast")}" '
    )
    svg = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}" '
        f'width="{RENDERED_EDGE_PIXELS}" height="{RENDERED_EDGE_PIXELS}">'
        f"{shapes}</svg>"
    )

    with tempfile.TemporaryDirectory() as scratch:
        rendered = Path(scratch) / "plateforce-1024.png"
        render_to_png(svg, rendered)
        subprocess.run(
            ["cargo", "tauri", "icon", str(rendered)],
            cwd=REPOSITORY / "src-tauri",
            check=True,
        )

    # `cargo tauri icon` also writes the Android and iOS launcher sets. No bundler this
    # project ships reads them. The Square*Logo and StoreLogo files beside them are read by
    # the Windows Store package and stay.
    icons = REPOSITORY / "src-tauri/icons"
    for mobile in ("android", "ios"):
        shutil.rmtree(icons / mobile, ignore_errors=True)

    written = sorted(path.name for path in icons.iterdir())
    print(f"rendered {len(written)} icon files from the header mark at {view_box}")


if __name__ == "__main__":
    sys.exit(main())
