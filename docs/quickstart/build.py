#!/usr/bin/env python3
"""Builds the quick start guides, one per route, and renders each to PDF.

The facts a guide states about the software are read out of the software rather than typed
here, so a guide cannot claim a version or a registry the build does not carry.

Usage: python3 docs/quickstart/build.py [--html-only]
"""

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))

import content  # noqa: E402

MARK = (
    '<svg class="cover__mark" viewBox="0 0 32 32" xmlns="http://www.w3.org/2000/svg">'
    '<rect width="32" height="32" rx="6" fill="#0a6a6f"/>'
    '<path d="M4 22 L11 22 L14 9 L18 26 L21 17 L28 17" fill="none" stroke="#ffffff" '
    'stroke-width="2.5" stroke-linejoin="round" stroke-linecap="round"/></svg>'
)

BROWSER_URL = "https://dralexharrison.github.io/plateforce/app/"


def facts():
    """The version and registry this build carries, read from the built program."""
    binary = ROOT / "target" / "debug" / "plateforce"
    if not binary.exists():
        binary = ROOT / "target" / "release" / "plateforce"
    version = json.loads(
        subprocess.run(
            [str(binary), "version", "--format", "json"],
            capture_output=True, text=True, check=True,
        ).stdout
    )["ok"]["plateforce_version"]
    return {
        "version": version,
        "registry_revision": (ROOT / "registry" / "VERSION").read_text().strip(),
        "registry_digest": digest(binary),
    }


def digest(binary):
    """The registry digest the build reports, which identifies the rule set exactly."""
    reported = json.loads(
        subprocess.run(
            [
                str(binary), "analyse",
                str(ROOT / "crates/plateforce-conformance/fixtures/subject01_trial1.force.txt"),
                "--column", "0", "--sentinel", "none", "--sample-rate-hz", "1200",
                "--preset", "sams", "--format", "json",
            ],
            capture_output=True, text=True, check=True,
        ).stdout
    )
    return reported["ok"]["registry_digest"]


def page(title, subtitle, lead, contents, body):
    items = "".join(f"<li>{item}</li>" for item in contents)
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{title}</title>
<link rel="stylesheet" href="style.css">
</head>
<body>
<header class="cover">
{MARK}
<h1>{title}</h1>
<p class="cover__product">{subtitle}</p>
<p class="cover__lead">{lead}</p>
<div class="cover__contents">
<h2>In this guide</h2>
<ol>{items}</ol>
</div>
</header>
<main>
{body}
</main>
</body>
</html>
"""


BROWSER_ENTRY = content.step(
    1,
    "Open your trial",
    """
<p>Drag your file onto the page, or press <strong>Choose a file</strong>. A whole folder of
trials goes in the same way with <strong>Choose a folder</strong>.</p>
"""
    + content.figure("open", "The first screen, and everything it offers."),
)


DESKTOP_ENTRY = content.step(
    1,
    "Open your trial",
    """
<p>Drag your file onto the window, or press <strong>Choose a file</strong>. A whole folder of
trials goes in the same way with <strong>Choose a folder</strong>.</p>
"""
    + content.figure("open", "The first screen, and everything it offers."),
)


def browser_open():
    return f"""
<h2>Open it</h2>

<p>Go to <a href="{BROWSER_URL}">{BROWSER_URL}</a> in Chrome, Edge, Firefox or Safari.</p>

<p>There is nothing to install and no account to make. Your file is read inside the browser
tab, on your own machine, and is never uploaded. The header says so while you work.</p>
"""


def contents_list():
    return [
        "What it does",
        "Before you start",
        "The first five minutes",
        "Getting your numbers out",
        "What to write in your methods section",
        "When something does not work",
        "Words you will meet",
    ]


def browser_guide(f):
    body = (
        content.what_it_is()
        + browser_open()
        + content.before_you_start()
        + content.the_five_minutes(BROWSER_ENTRY)
        + content.getting_numbers_out()
        + content.methods_section().format(**f)
        + content.troubleshooting()
        + content.glossary()
        + content.elsewhere("browser")
        + footer(f)
    )
    return page(
        "Quick Start",
        "plateforce in your web browser",
        "Read a force trace, get the jump numbers, and keep the record of how they were "
        "computed.",
        ["Open it"] + contents_list() + ["Where else it runs"],
        body,
    )


DESKTOP = {
    "macos": ("plateforce for macOS", "Install it"),
    "windows": ("plateforce for Windows", "Install it"),
    "linux": ("plateforce for Linux", "Install it"),
}


def desktop_guide(platform):
    def make(f):
        body = (
            content.what_it_is()
            + content.install(platform).format(
                version=f["version"],
                releases="https://github.com/DrAlexHarrison/plateforce/releases",
            )
            + content.before_you_start()
            + content.the_five_minutes(DESKTOP_ENTRY)
            + content.getting_numbers_out()
            + content.methods_section().format(**f)
            + content.troubleshooting()
            + content.glossary()
            + content.elsewhere("desktop")
            + footer(f)
        )
        return page(
            "Quick Start",
            DESKTOP[platform][0],
            "Read a force trace, get the jump numbers, and keep the record of how they were "
            "computed.",
            ["Install it"] + contents_list() + ["Where else it runs"],
            body,
        )

    return make


def footer(f):
    return f"""
<p class="footer-note">plateforce {f['version']}, method registry {f['registry_revision']}.
Apache-2.0. Source and registry at github.com/DrAlexHarrison/plateforce.</p>
"""


TERMINAL = {
    "terminal": ("any", "plateforce at a terminal"),
    "terminal-macos": ("macos", "plateforce at a terminal on macOS"),
    "terminal-windows": ("windows", "plateforce at a terminal on Windows"),
    "terminal-linux": ("linux", "plateforce at a terminal on Linux"),
}


def terminal_guide(shell="any", subtitle="plateforce at a terminal"):
    """The terminal guides are authored as Markdown, because the readers who need them most
    are a terminal and an assistant, and a PDF is unreadable to both. One source per shell,
    two outputs each, and only the section about getting the program differs."""

    def make(f, markdown_into=None):
        source = (HERE / "terminal.md").read_text().replace(
            "<!--GET-THE-PROGRAM-->", content.TERMINAL_INSTALL[shell]
        )
        # Written outside the tree: it is derived from terminal.md, and a generated copy
        # beside its source is a second place for the same words to be corrected. A caller
        # naming a directory gets the Markdown itself, which is what a terminal and an
        # assistant read, and is how these reach a release without becoming a second source.
        into = Path(markdown_into) if markdown_into else Path(
            tempfile.mkdtemp(prefix="plateforce-quickstart-")
        )
        into.mkdir(parents=True, exist_ok=True)
        written = into / f"quick-start-terminal-{shell}.md"
        written.write_text(source)
        # A block carries its language so the command checker knows a shell line from printed
        # output; highlighting it would make those blocks the only coloured ones on the page.
        rendered = subprocess.run(
            ["pandoc", "--from", "gfm", "--to", "html", "--no-highlight", str(written)],
            capture_output=True, text=True, check=True,
        ).stdout
        # The Markdown carries its own title block for a reader who opens the file
        # directly, and the page template carries one too, so the fragment gives up its two
        # headings and the one sentence the cover already prints.
        body = rendered.split("</h2>", 1)[1] if "</h2>" in rendered else rendered
        body = body.split("</p>", 1)[1] if "</p>" in body else body
        return page(
            "Quick Start",
            subtitle,
            "Read a force trace, get the jump numbers, and keep the record of how they were "
            "computed.",
            [
                "What it does", "Get the program",
                "What plateforce needs to know about your file",
                "One trial", "Reading what comes back", "A folder of trials",
                "If you would rather click", "For an assistant, or a script",
                "When something does not work",
            ],
            body + footer(f),
        )

    return make


GUIDES = {
    "browser": browser_guide,
    "macos": desktop_guide("macos"),
    "windows": desktop_guide("windows"),
    "linux": desktop_guide("linux"),
    **{name: terminal_guide(shell, subtitle)
       for name, (shell, subtitle) in TERMINAL.items()},
}


def render_pdf(html_path, pdf_path):
    """Prints the page with headless Chrome, which is the only renderer here that reads the
    print stylesheet the guide is designed against."""
    profile = tempfile.mkdtemp(prefix="plateforce-quickstart-", dir="/dev/shm")
    try:
        subprocess.run(
            [
                "google-chrome", "--headless=new", "--disable-gpu", "--no-sandbox",
                f"--user-data-dir={profile}",
                "--no-pdf-header-footer", "--print-to-pdf-no-header",
                f"--print-to-pdf={pdf_path}", str(html_path),
            ],
            capture_output=True, text=True, check=True, timeout=120,
        )
    finally:
        shutil.rmtree(profile, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--html-only", action="store_true")
    parser.add_argument("--only", default=None)
    parser.add_argument(
        "--markdown-into",
        default=None,
        help="where to keep the terminal guides as Markdown, which is what a terminal and an "
        "assistant read",
    )
    arguments = parser.parse_args()

    f = facts()
    built = []
    for name, make in GUIDES.items():
        if arguments.only and arguments.only != name:
            continue
        html_path = HERE / f"quick-start-{name}.html"
        html_path.write_text(
            make(f, arguments.markdown_into) if name in TERMINAL else make(f)
        )
        built.append(html_path)
        if not arguments.html_only:
            pdf_path = HERE / f"quick-start-{name}.pdf"
            render_pdf(html_path, pdf_path)
            print(f"{pdf_path}  {pdf_path.stat().st_size // 1024} KB")
        else:
            print(html_path)


if __name__ == "__main__":
    main()
