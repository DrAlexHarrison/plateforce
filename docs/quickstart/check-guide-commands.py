#!/usr/bin/env python3
"""Every command line the guides print, checked against the program that runs it.

A command in a document that does not run is the defect this project exists to prevent
wearing a different hat, so the fenced blocks, which are the surface a reader pastes, are
accounted for line by line rather than sampled. An untagged block is POSIX shell, a
`powershell` block is PowerShell, and a `text` block is output this program printed. Every
line of an untagged block is either one of this program's own commands or one of the install
commands the guides name, and a line that is neither fails the run rather than passing
unseen. This program's own lines are run against a real trial in a scratch folder, and each
must reach the exit code the guide states beneath it, which is 0 wherever the guide states
none.

    python3 docs/quickstart/check-guide-commands.py
"""

import json
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))

import content  # noqa: E402

FIXTURES = ROOT / "crates/plateforce-conformance/fixtures"

# A floor under the count this program's own lines reach, and a shape check below it. Either
# alone lets a broken extractor certify a clean sweep; the classification identity is what
# makes the count a denominator rather than a sample.
FLOOR = 9

# The install commands the guides name. A line reaching for anything else is a line nobody
# decided to publish, so it stops the run.
INSTALL_COMMANDS = {"curl", "chmod", "xattr", "mkdir", "mv", "echo", "export"}

# Both separators, because the desktop bundles are named with underscores and the command line
# programs with hyphens, and a pattern carrying one of them reads the other guide's file names
# as prose.
ASSET = re.compile(r"plateforce[-_][\w.-]*(?<!\.)\b")
PROGRAM = re.compile(r"^(plateforce|\./plateforce-[\w.-]+|\.\\plateforce-[\w.-]+\.exe)$")
PLACEHOLDER = re.compile(r"<[A-Za-z][\w-]*>")
BLOCK = re.compile(r"```(\w*)\n(.*?)```(.*?)(?=```|\Z)", re.S)
STATED_EXIT = re.compile(r"\bexits (\d+)\b")
DECLARED = re.compile(r"local declared=\(\n(.*?)\n\s*\)", re.S)


def version_of(binary):
    """The version this build carries, read from it the way the guides read theirs."""
    return json.loads(
        subprocess.run(
            [str(binary), "version", "--format", "json"],
            capture_output=True, text=True, check=True,
        ).stdout
    )["ok"]["plateforce_version"]


def release_assets(version):
    """The names a release publishes, read from the script that counts them.

    Restating them here would make this gate agree with itself while a reader followed a guide
    to a file the release does not carry. The nine names are one list, in
    `scripts/verify-release-artefacts.sh`, and this reads that list.
    """
    declared = DECLARED.search((ROOT / "scripts/verify-release-artefacts.sh").read_text())
    if not declared:
        return set()
    return {
        name.replace("${version}", version)
        for name in re.findall(r'"([^"]+)"', declared.group(1))
    }


def guide_source():
    """The Markdown the terminal guides are built from, every shell's install section with it."""
    return (HERE / "terminal.md").read_text() + "\n" + "\n".join(
        content.TERMINAL_INSTALL.values()
    )


def logical_lines(body):
    """One entry per command, with backslash continuations joined back together."""
    joined = body.replace("\\\n", " ")
    return [" ".join(line.split()) for line in joined.splitlines() if line.strip()]


def blocks():
    """Every fenced block, as its language, its logical lines, its raw lines, and the prose
    beneath it."""
    return [
        (language, logical_lines(body), body.splitlines(), following)
        for language, body, following in BLOCK.findall(guide_source())
    ]


def classify(line):
    """Which of the three kinds of line this is, or nothing when it is none of them."""
    try:
        first = shlex.split(line)[0]
    except ValueError:
        return None
    if PROGRAM.match(first):
        return "program"
    if first in INSTALL_COMMANDS:
        return "install"
    return None


def named_assets(text):
    """Every release asset name the text reaches for."""
    return set(ASSET.findall(text)) - {"plateforce-", "plateforce_"}


def desktop_download_sections(version):
    """The three sections that tell a reader which installer to download, as rendered.

    The fenced blocks below are the terminal guides, and the file name a desktop reader is sent
    for appears in none of them: it is prose in `content.install`, and until this read it, the
    only thing standing between a renamed bundle and a 404 was somebody remembering.
    """
    return {
        platform: content.install(platform).format(
            version=version, releases=content.RELEASES
        )
        for platform in ("macos", "windows", "linux")
    }


def scratch_folder(into):
    """A folder shaped like the one the guides describe, so what runs is what a reader pastes."""
    shutil.copy(FIXTURES / "subject01_trial1.force.txt", into / "jump.txt")
    (into / "trials").mkdir()
    for trial in range(1, 7):
        shutil.copy(
            FIXTURES / f"subject01_trial{trial}.force.txt", into / "trials" / f"AT01_{trial}.txt"
        )


def substitute(argument, scratch, run_index):
    """The reader's own names, pointed at the scratch folder."""
    if argument == "trial.txt":
        return str(scratch / "jump.txt")
    if argument == "trials/":
        return str(scratch / "trials")
    if argument == "results":
        return str(scratch / f"out-{run_index}")
    return argument


def read_the_guides(published):
    """The lines to run, and every complaint about the blocks they came from."""
    to_run, complaints = [], []
    longest_physical = 0
    for language, lines, raw, following in blocks():
        longest_physical = max([longest_physical] + [len(line.rstrip(" \\")) for line in raw])
        text = "\n".join(lines)
        if language != "text":
            for placeholder in PLACEHOLDER.findall(text):
                complaints.append(f"{placeholder} is a placeholder a reader cannot paste")
            for asset in named_assets(text) - published:
                complaints.append(f"{asset} is not a name the release publishes")
        if language == "text":
            continue
        stated = STATED_EXIT.search(following)
        expected = int(stated.group(1)) if stated else 0
        for line in lines:
            if language == "powershell":
                # PowerShell continues inside brackets rather than at the line, so a line of
                # one of its blocks is not a command on its own. This program's own lines are.
                if classify(line) == "program":
                    to_run.append((line, expected))
                continue
            kind = classify(line)
            if kind is None:
                complaints.append(f"unaccounted for, so nothing checked it: {line}")
            elif kind == "program":
                to_run.append((line, expected))
    return to_run, complaints, longest_physical


def shell_blocks_parse():
    """Every untagged block, read by the shell a reader pastes it into."""
    complaints = []
    for language, lines, _, _ in blocks():
        if language:
            continue
        source = "\n".join(lines)
        parsed = subprocess.run(
            ["bash", "-n"], input=source, capture_output=True, text=True
        )
        if parsed.returncode != 0:
            complaints.append(f"a shell will not parse this block: {parsed.stderr.strip()}")
    return complaints


def run_one(binary, line, expected, scratch, run_index):
    """One line, run where a reader would run it. Its exit code, and what to say about it."""
    arguments = [substitute(a, scratch, run_index) for a in shlex.split(line)[1:]]
    serving = arguments[:1] == ["serve"]
    try:
        # The generous timeout is for a debug binary on a loaded machine, not for the program.
        # `serve` holds the terminal open by design, so it is asked to start and then stopped:
        # reaching the timeout is the pass, since a broken one exits at once.
        run = subprocess.run(
            [str(binary), *arguments], capture_output=True, text=True, timeout=120
        )
    except subprocess.TimeoutExpired:
        return (None, None) if serving else (None, f"hung rather than exiting: {line}")
    if serving:
        return run.returncode, f"exited {run.returncode} rather than serving: {line}"
    if run.returncode != expected:
        first = (run.stderr or run.stdout).strip().splitlines()
        return run.returncode, (
            f"exited {run.returncode} where the guide states {expected}: {line}\n"
            f"    {first[0][:140] if first else ''}"
        )
    return run.returncode, None


def main():
    binary = ROOT / "target/debug/plateforce"
    if not binary.exists():
        print(f"no built program at {binary}", file=sys.stderr)
        return 1

    version = version_of(binary)
    published = release_assets(version)
    # A set read out of a script that stopped declaring one would make every name below
    # unpublishable, and a set read out of a script that stopped declaring the version would
    # make every name below publishable. Neither reads as a broken reader, so both are stated.
    if len(published) < 9 or not any(version in name for name in published):
        print(
            f"{len(published)} release assets read from scripts/verify-release-artefacts.sh, "
            f"{len([name for name in published if version in name])} of them naming {version}, "
            "so the names below were compared against a list that is not the release's",
            file=sys.stderr,
        )
        return 1

    to_run, complaints, longest_physical = read_the_guides(published)
    complaints += shell_blocks_parse()

    # The file a desktop reader is sent to download, which appears in no fenced block.
    downloads = 0
    for platform, section in desktop_download_sections(version).items():
        named = named_assets(section)
        downloads += len(named)
        for asset in named - published:
            complaints.append(f"the {platform} guide sends a reader for {asset}, which the "
                              "release does not publish")
        if not named:
            complaints.append(f"the {platform} guide names no file to download")

    lines = [line for line, _ in to_run]
    if len(lines) < FLOOR:
        complaints.append(
            f"{len(lines)} of this program's command lines extracted, under the floor of "
            f"{FLOOR}, so the extractor is reading less than the guides print"
        )
    for shape in ("analyse", "batch", "serve"):
        if not any(f" {shape} " in f"{line} " for line in lines):
            complaints.append(f"no {shape} line among the {len(lines)} extracted")
    # A joined command is longer than the longest line it was written across, so a run whose
    # longest command fits inside one printed line is one that dropped a continuation.
    if lines and max(len(line) for line in lines) <= longest_physical:
        complaints.append(
            "no extracted command outruns a single printed line, so continuations were dropped"
        )

    if complaints:
        for complaint in complaints:
            print(f"  {complaint}", file=sys.stderr)
        print(f"{len(complaints)} complaints about the blocks the guides print", file=sys.stderr)
        return 1

    produced = broken = 0
    with tempfile.TemporaryDirectory() as folder:
        scratch = Path(folder)
        scratch_folder(scratch)
        for run_index, (line, expected) in enumerate(to_run):
            _, complaint = run_one(binary, line, expected, scratch, run_index)
            if complaint:
                broken += 1
                print(f"BROKEN: {complaint}", file=sys.stderr)
            else:
                produced += 1

    print(
        f"{len(to_run)} of this program's command lines extracted from the guides, "
        f"{produced} of {len(to_run)} reached the exit code stated for them, {broken} did not"
    )
    print(
        f"{downloads} file names across the three desktop guides, and every name any guide "
        f"prints is one of the {len(published)} the release publishes"
    )
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main())
