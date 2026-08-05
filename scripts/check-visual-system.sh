#!/usr/bin/env bash

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

python3 - <<'CHECKS'
import collections
import re
import sys
from pathlib import Path

stylesheet = Path("web/styles.css")
chart = Path("web/chart.js")
dimension_allowlist = Path("scripts/visual-dimensions.txt")
source = stylesheet.read_text()

# Keep offsets and line numbers intact while making comments invisible to every parser below.
css = re.sub(r"/\*.*?\*/", lambda match: re.sub(r"[^\n]", " ", match.group(0)), source, flags=re.S)
failures = []


def line_at(offset):
    return css.count("\n", 0, offset) + 1


def fail(offset, message):
    failures.append(f"{stylesheet}:{line_at(offset)} {message}")


# Identify every :root block, including one nested in a media query.
root_intervals = []
stack = []
boundary = 0
for index, character in enumerate(css):
    if character == "{":
        header = css[boundary:index].strip()
        stack.append((header, index + 1))
        boundary = index + 1
    elif character == "}":
        if stack:
            header, start = stack.pop()
            if ":root" in header:
                root_intervals.append((start, index))
        boundary = index + 1
    elif character == ";":
        boundary = index + 1


def in_root(offset):
    return any(start <= offset < end for start, end in root_intervals)


declarations = []
for match in re.finditer(r"([\w-]+)\s*:\s*([^;{}]+);", css):
    declarations.append((match.start(), match.group(1), match.group(2).strip()))


# Colour literals belong to the token blocks. A control count proves the matcher saw them there.
colour = re.compile(r"#[0-9a-fA-F]{3,8}\b|(?:rgba?|hsla?)\([^)]*\)")
root_colours = 0
for match in colour.finditer(css):
    if in_root(match.start()):
        root_colours += 1
    else:
        fail(match.start(), f"colour literal {match.group(0)!r} is outside a :root token block")
if root_colours == 0:
    failures.append("colour check found no token literals, so it measured nothing")


# Type uses the declared scale in CSS and on the canvas.
type_declarations = 0
for offset, property_name, value in declarations:
    if property_name != "font-size" or in_root(offset):
        continue
    type_declarations += 1
    if not re.fullmatch(r"var\(--text-[\w-]+\)", value):
        fail(offset, f"font-size {value!r} is outside the type scale")
canvas_fonts = [
    (number, line)
    for number, line in enumerate(chart.read_text().splitlines(), start=1)
    if "context.font" in line
]
if not canvas_fonts:
    failures.append(f"{chart} carries no canvas font assignment, so its type was not checked")
for number, line in canvas_fonts:
    if re.search(r"\b\d+(?:\.\d+)?(?:px|rem|em)\b", line):
        failures.append(f"{chart}:{number} canvas font contains a freehand size: {line.strip()}")


# Spacing accepts the scale, zero, auto and negative centring offsets. Positive literals fail.
spacing_properties = re.compile(r"^(?:padding(?:-[\w-]+)?|margin(?:-[\w-]+)?|gap|row-gap|column-gap)$")
length = re.compile(r"(?<![\w-])(-?\d+(?:\.\d+)?)(px|rem|em)\b")
spacing_declarations = 0
for offset, property_name, value in declarations:
    if not spacing_properties.match(property_name) or in_root(offset):
        continue
    spacing_declarations += 1
    for measured in length.finditer(value):
        amount = float(measured.group(1))
        if amount > 0:
            fail(offset, f"{property_name} contains freehand spacing {measured.group(0)!r}")


# One component radius and one full pill. Circles may use 50 percent.
radius_declarations = 0
allowed_radii = {"var(--radius)", "var(--radius-pill)", "50%"}
for offset, property_name, value in declarations:
    if property_name != "border-radius" or in_root(offset):
        continue
    radius_declarations += 1
    if value not in allowed_radii:
        fail(offset, f"border-radius {value!r} is outside the radius system")
declared_radii = {
    property_name
    for offset, property_name, _ in declarations
    if in_root(offset) and property_name in {"--radius", "--radius-pill"}
}
if declared_radii != {"--radius", "--radius-pill"}:
    failures.append(f"radius tokens are {sorted(declared_radii)}, expected --radius and --radius-pill")


# A hairline shadow separates sticky headings. The only elevation is the shared shadow token.
elevation_declarations = 0
for offset, property_name, value in declarations:
    if in_root(offset):
        continue
    if property_name == "box-shadow":
        if value == "var(--shadow)":
            elevation_declarations += 1
        elif value != "0 1px 0 var(--border-strong)":
            fail(offset, f"box-shadow {value!r} is a second elevation device")
    if property_name == "filter" and "drop-shadow" in value:
        fail(offset, "drop-shadow is a second elevation device")
if elevation_declarations == 0:
    failures.append("elevation check found no use of --shadow, so it measured nothing")


# Pixel dimensions are deliberate geometry and stay stable behind a counted, reasoned allowlist.
dimension_properties = {
    "width", "height", "min-width", "max-width", "min-height", "max-height",
    "top", "right", "bottom", "left", "inset",
}
dimensions = collections.Counter()
for offset, property_name, value in declarations:
    if in_root(offset):
        continue
    if property_name in dimension_properties and re.search(r"-?\d+(?:\.\d+)?px\b", value):
        dimensions[f"{property_name}: {' '.join(value.split())}"] += 1
    if property_name.startswith("margin-") and re.search(r"-\d+(?:\.\d+)?px\b", value):
        dimensions[f"{property_name}: {' '.join(value.split())}"] += 1

allowed_dimensions = collections.Counter()
reasons = {}
if not dimension_allowlist.exists():
    failures.append(f"{dimension_allowlist} is missing")
else:
    for number, raw in enumerate(dimension_allowlist.read_text().splitlines(), start=1):
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        parts = raw.split("\t")
        if len(parts) != 3 or not parts[1].isdigit() or not parts[2].strip():
            failures.append(f"{dimension_allowlist}:{number} expected declaration, count and reason")
            continue
        declaration, count, reason = parts
        allowed_dimensions[declaration] = int(count)
        reasons[declaration] = reason

for declaration in sorted(set(dimensions) | set(allowed_dimensions)):
    actual = dimensions[declaration]
    allowed = allowed_dimensions[declaration]
    if actual != allowed:
        failures.append(
            f"dimension {declaration!r} appears {actual} times, allowlist says {allowed}" +
            (f" ({reasons[declaration]})" if declaration in reasons else " and gives no reason")
        )


if failures:
    for message in failures:
        print(f"FAIL  {message}")
    print(f"\n{len(failures)} visual-system violations")
    sys.exit(1)

print(f"pass  colours: {root_colours} literals, all inside token blocks")
print(f"pass  type: {type_declarations} CSS uses and {len(canvas_fonts)} canvas use, all on the scale")
print(f"pass  spacing: {spacing_declarations} declarations, no positive freehand lengths")
print(f"pass  radius: {radius_declarations} uses, one component radius plus pills and circles")
print(f"pass  elevation: {elevation_declarations} elevated surfaces, one shared device")
print(f"pass  dimensions: {sum(dimensions.values())} literal uses, each counted with a reason")
CHECKS
