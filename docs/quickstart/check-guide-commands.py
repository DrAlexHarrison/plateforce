#!/usr/bin/env python3
"""Every command line the guides print, run against a real trial.

A command in a document that does not run is the defect this project exists to prevent
wearing a different hat, so the guides are held to the bar the help pages are held to:
extracted mechanically, run in a scratch folder, and counted against a floor, so a broken
extractor cannot certify a clean sweep.

    python3 docs/quickstart/check-guide-commands.py
"""

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))

import content  # noqa: E402

# The guides print five of this program's command lines. A floor plus the shape check
# below is what stops a broken extractor certifying a clean sweep.
FLOOR = 5
FIXTURES = ROOT / "crates/plateforce-conformance/fixtures"


def command_lines():
    """Every `plateforce ...` line inside a fenced block, in the guides that print commands.

    Fenced blocks only: a command named inside a sentence is prose about a command, and the
    reader does not paste it.
    """
    text = (HERE / "terminal.md").read_text()
    text += "\n".join(content.TERMINAL_INSTALL.values())
    found = []
    for block in re.findall(r"```\n(.*?)```", text, re.S):
        for line in block.replace("\\\n", " ").splitlines():
            line = " ".join(line.split())
            if line.startswith("plateforce ") and not line.startswith("plateforce:"):
                found.append(line)
    return sorted(set(found))


def scratch_folder(into):
    """A folder shaped like the one the guides describe, so what runs is what a reader pastes."""
    shutil.copy(FIXTURES / "subject01_trial1.force.txt", into / "jump.txt")
    (into / "trials").mkdir()
    for trial in (1, 2, 3):
        shutil.copy(FIXTURES / f"subject01_trial{trial}.force.txt", into / "trials")


def main():
    binary = ROOT / "target/debug/plateforce"
    if not binary.exists():
        print(f"no built program at {binary}", file=sys.stderr)
        return 1

    lines = command_lines()
    if len(lines) < FLOOR:
        print(f"extracted {len(lines)} commands, under the floor of {FLOOR}, so the "
              "extractor is broken", file=sys.stderr)
        return 1
    for shape in ("analyse", "batch", "serve"):
        if not any(line.startswith(f"plateforce {shape}") for line in lines):
            print(f"no {shape} line among the {len(lines)} extracted, so the extractor is "
                  "reading the wrong thing", file=sys.stderr)
            return 1

    produced = declined = broken = 0
    with tempfile.TemporaryDirectory() as folder:
        scratch = Path(folder)
        scratch_folder(scratch)
        for line in lines:
            arguments = line.split()[1:]
            arguments = [
                str(scratch / "jump.txt") if a == "trial.txt"
                else str(scratch / "trials") if a == "trials/"
                else str(scratch / "out") if a == "results"
                else a
                for a in arguments
            ]
            # The download URLs and file placeholders in the install sections are not this
            # program's arguments.
            if any(a.startswith("<") or a.startswith("http") for a in arguments):
                continue
            # The generous timeout is for a debug binary on a loaded machine, not for the program.
            # `serve` holds the terminal open by design, so it is asked to start and then
            # stopped: reaching the timeout is the pass, since a broken one exits at once.
            serving = arguments[:1] == ["serve"]
            try:
                run = subprocess.run([str(binary), *arguments], capture_output=True,
                                     text=True, timeout=120)
            except subprocess.TimeoutExpired:
                if serving:
                    produced += 1
                    continue
                broken += 1
                print(f"BROKEN (hung): {line}", file=sys.stderr)
                continue
            if serving:
                broken += 1
                print(f"BROKEN (serve exited {run.returncode} rather than serving): {line}",
                      file=sys.stderr)
                continue
            if run.returncode == 0:
                produced += 1
            elif run.returncode in (64, 65):
                declined += 1
                print(f"  declined ({run.returncode}): {line}")
            else:
                broken += 1
                print(f"BROKEN ({run.returncode}): {line}", file=sys.stderr)
                print("   " + (run.stderr or run.stdout).strip().splitlines()[0][:140],
                      file=sys.stderr)

    print(f"{len(lines)} command lines extracted: {produced} produced a result, "
          f"{declined} declined by a published refusal, {broken} broken")
    if broken:
        return 1
    if not produced:
        print("nothing produced a result, so this check proved nothing", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
