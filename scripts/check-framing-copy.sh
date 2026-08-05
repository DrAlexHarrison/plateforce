#!/usr/bin/env bash
#
# Three rules about the words a reader meets, checked mechanically.
#
# 1. The debate count is not the opening claim and never appears without its denominator.
#    "119 genuine methodological debates" reads to a first-year as a statement about their
#    qualifications. The consequence is the claim: ten published ways of computing one jump
#    height disagree by more than the training effect the source study was built to detect.
#    The count is read from the registry rather than written here, so the check follows the
#    data instead of going stale against it.
#
# 2. The words a first-year does not use do not appear in anything they read as a label.
#    Across six university course documents `onset`, `threshold`, `epoch`, `filter`,
#    `provenance` and `fingerprint` each appear in 0 of 6. They are legal as registry
#    identifiers, which are shown deliberately and on the same row as the label, and they
#    are illegal in the label itself.
#
# 3. No string a reader meets describes the state of the software. `CONVENTIONS.md`
#    section 5, which reaches a comment as well as a string, so the two are read apart and
#    reported apart. This one covers `web/` only: coverage limits belong in README.md,
#    where somebody deciding whether to adopt this looks for them.
#
# Every arm reports what it read as well as whether it passed, and no arm may pass while
# reading nothing. A gate that cannot fail is worth less than no gate, because it converts
# an unchecked property into a reported-green one.
#
# Usage: scripts/check-framing-copy.sh

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

# Rule 1 takes the count from the registry rather than carrying one, so it needs the
# command to run. When it cannot, say which command and why rather than exiting on its
# status, which reads as this check being broken.
if ! census="$(cargo run -q -p plateforce-cli -- registry census 2>&1)"; then
  printf 'FAIL  registry census could not run, so the debate count rule has no count to check\n'
  printf '%s\n' "$census" | tail -5
  exit 1
fi

python3 - "$census" <<'CHECKS'
import re
import sys
from pathlib import Path

census = sys.argv[1]
failures = []


def report(where, message):
    failures.append(f"{where} {message}")


# ------------------------------------------------- telling a string from a comment apart

# A regular expression cannot do this. The two things below each contain the other's
# delimiters routinely, so a pattern for either one reads the other's contents as its own:
# the phrase that prompted this rescope sat in a comment and was reported as interface copy,
# and a phrase a concatenation builds was invisible to the same scan in either arrangement.

# After one of these a slash opens a pattern rather than dividing. Anywhere else it divides,
# and a pattern holding a quote would otherwise open a string that runs to the end of file.
REGEX_MAY_FOLLOW_CHARACTER = set("(,=:[!&|?{};+-*%~^<>")
REGEX_MAY_FOLLOW_WORD = {"return", "typeof", "case", "in", "of", "do", "else", "yield",
                         "await", "new", "delete", "void", "instanceof"}


def split_javascript(source):
    """Every quoted string and every comment in a JavaScript source.

    A string carries the offsets it spans, so literals a `+` joins can be read afterwards as
    the one sentence a reader meets.
    """
    strings, comments = [], []
    index, length, line = 0, len(source), 1
    previous_character, previous_word = "", ""
    while index < length:
        character = source[index]
        if character == "\n":
            line += 1
            index += 1
            continue
        if character == "/" and index + 1 < length and source[index + 1] == "/":
            end = source.find("\n", index)
            end = length if end < 0 else end
            comments.append((line, source[index:end]))
            index = end
            continue
        if character == "/" and index + 1 < length and source[index + 1] == "*":
            end = source.find("*/", index + 2)
            end = length if end < 0 else end + 2
            comments.append((line, source[index:end]))
            line += source.count("\n", index, end)
            index = end
            continue
        if character == "/" and (previous_character in REGEX_MAY_FOLLOW_CHARACTER
                                 or previous_word in REGEX_MAY_FOLLOW_WORD
                                 or previous_character == ""):
            end, in_class = index + 1, False
            while end < length:
                if source[end] == "\\":
                    end += 2
                    continue
                if source[end] == "[":
                    in_class = True
                elif source[end] == "]":
                    in_class = False
                elif source[end] == "/" and not in_class:
                    break
                elif source[end] == "\n":
                    break
                end += 1
            line += source.count("\n", index, min(end + 1, length))
            index = min(end + 1, length)
            previous_character, previous_word = "/", ""
            continue
        if character in "'\"`":
            start, end = index, index + 1
            while end < length:
                if source[end] == "\\":
                    end += 2
                    continue
                if source[end] == character:
                    break
                end += 1
            strings.append((line, start, min(end + 1, length), source[start + 1:end]))
            line += source.count("\n", start, min(end + 1, length))
            index = min(end + 1, length)
            previous_character, previous_word = character, ""
            continue
        if character.isalnum() or character in "_$":
            end = index
            while end < length and (source[end].isalnum() or source[end] in "_$"):
                end += 1
            previous_word = source[index:end]
            previous_character = source[end - 1]
            index = end
            continue
        if not character.isspace():
            previous_character, previous_word = character, ""
        index += 1
    return strings, comments


def joined_by_concatenation(source, strings):
    """Literals a `+` joins, read as the one string they build."""
    joined, index = [], 0
    while index < len(strings):
        line, text = strings[index][0], strings[index][3]
        while index + 1 < len(strings):
            between = source[strings[index][2]:strings[index + 1][1]]
            if "+" not in between or between.strip(" \t\r\n+"):
                break
            index += 1
            text += strings[index][3]
        joined.append((line, text))
        index += 1
    return joined


HTML_COMMENT = re.compile(r"<!--.*?-->", re.S)
# What a reader reads or a screen reader speaks. `title` and `alt` join the two the label
# rule already reads, because a string nobody sees on the page is still a string a reader
# meets.
READER_ATTRIBUTES = ("placeholder", "aria-label", "title", "alt")


def split_markup(source):
    """The text between the tags and the attributes a reader reads, apart from the comments."""
    comments = [(source.count("\n", 0, match.start()) + 1, match.group(0))
                for match in HTML_COMMENT.finditer(source)]
    # Blanked rather than removed, so every offset below still names the line it came from.
    blanked = HTML_COMMENT.sub(lambda match: re.sub(r"[^\n]", " ", match.group(0)), source)
    strings = []
    for match in re.finditer(r">([^<]+)<", blanked):
        if match.group(1).strip():
            strings.append((blanked.count("\n", 0, match.start(1)) + 1, match.group(1).strip()))
    for name in READER_ATTRIBUTES:
        for match in re.finditer(rf'\b{name}="([^"]*)"', blanked):
            if match.group(1).strip():
                strings.append((blanked.count("\n", 0, match.start(1)) + 1, match.group(1)))
    return strings, comments


STYLESHEET_COMMENT = re.compile(r"/\*.*?\*/", re.S)


def split_stylesheet(source):
    """A stylesheet's quoted strings, which reach a reader through `content`, and its comments."""
    comments = [(source.count("\n", 0, match.start()) + 1, match.group(0))
                for match in STYLESHEET_COMMENT.finditer(source)]
    blanked = STYLESHEET_COMMENT.sub(lambda match: re.sub(r"[^\n]", " ", match.group(0)), source)
    strings = []
    for match in re.finditer(r"""(?:"([^"\n]*)"|'([^'\n]*)')""", blanked):
        # Either quote, read from whichever group took part. Reading only the first group
        # drops every single-quoted string, which is the quote this stylesheet uses.
        text = match.group(1) if match.group(1) is not None else match.group(2)
        if text.strip():
            strings.append((blanked.count("\n", 0, match.start()) + 1, text))
    return strings, comments


def split_source(path, source):
    if path.suffix == ".js":
        strings, comments = split_javascript(source)
        return joined_by_concatenation(source, strings), comments
    if path.suffix == ".html":
        return split_markup(source)
    return split_stylesheet(source)


# ------------------------------------------------------------------ 1. the count

match = re.search(r"genuine debates\s+(\d+) of (\d+)", census)
if match:
    debates, entries = match.group(1), match.group(2)
    bare = re.compile(rf"(?<!\d){debates}(?!\d)")

    # The matcher, proved against strings this rule owns before any reading of the interface
    # is believed. A count that stopped matching, or a denominator test that stopped
    # rejecting, reads exactly like a README that never printed the count.
    carries_denominator = f"{debates} of {entries} entries are genuine debates"
    bare_claim = f"{debates} genuine methodological debates"
    inside_a_longer_number = f"sampled at {debates}00 hz"
    matcher_faults = []
    if not bare.search(carries_denominator):
        matcher_faults.append(f"did not find {debates} in {carries_denominator!r}")
    if not bare.search(bare_claim):
        matcher_faults.append(f"did not find {debates} in {bare_claim!r}")
    if bare.search(inside_a_longer_number):
        matcher_faults.append(f"found {debates} inside the longer number in {inside_a_longer_number!r}")
    if f"of {entries}" in bare_claim:
        matcher_faults.append(f"read a denominator in {bare_claim!r}, which carries none")
    if matcher_faults:
        report("rule 1", f"the matcher failed its own control, so nothing it says about the "
                         f"interface can be believed: {'; '.join(matcher_faults)}")

    # Named rather than counted, so narrowing what rule 1 reads cannot read as an interface
    # that stopped printing the count.
    CARRY_THE_CLAIM = (Path("README.md"), Path("web/index.html"))
    missing = [path.as_posix() for path in CARRY_THE_CLAIM if not path.exists()]
    if missing:
        report("rule 1", f"cannot read {', '.join(missing)}, which is where the claim is made")

    checked = 0
    count_failures = len(matcher_faults) + len(missing)
    for path in CARRY_THE_CLAIM:
        if not path.exists():
            continue
        for number, line in enumerate(path.read_text().splitlines(), start=1):
            if not bare.search(line):
                continue
            checked += 1
            if f"of {entries}" not in line:
                count_failures += 1
                report(f"{path}:{number}", f"the debate count {debates} appears without its denominator: {line.strip()}")

    opening_paragraphs = Path("README.md").read_text().split("\n\n")[:2]
    opening = "\n".join(opening_paragraphs)
    if bare.search(opening):
        count_failures += 1
        report("README.md", f"opens on the debate count {debates} rather than on what it costs the reader")

    if not count_failures:
        where = ", ".join(path.as_posix() for path in CARRY_THE_CLAIM)
        if checked:
            print(f"pass  rule 1, the debate count carries its denominator in all {checked} lines "
                  f"that print it across {where}, and README.md does not open on it "
                  f"({debates} of {entries} read from registry census)")
        else:
            # Zero is a legitimate reading of this rule and a dangerous thing to print, so it
            # says which half of the rule had nothing to check rather than reporting coverage
            # it does not have.
            print(f"pass  rule 1, README.md does not open on the debate count: the count "
                  f"{debates} is not in its first {len(opening_paragraphs)} paragraphs. "
                  f"THE DENOMINATOR HALF CHECKED NOTHING, because {debates} appears on no line "
                  f"of {where} ({debates} of {entries} read from registry census)")
else:
    report("registry census", "did not report the debate count, so rule 1 cannot be checked")


# ------------------------------------------------------------- 2. the vocabulary

NOT_THEIR_WORD = ("onset", "threshold", "epoch", "filter", "provenance", "fingerprint")
banned = re.compile(rf"\b({'|'.join(NOT_THEIR_WORD)})\b", re.IGNORECASE)
# A registry identifier is shown on purpose, on the same row as the label, so a word
# inside one is not the label using it.
identifier = re.compile(r"\b[a-z_]+(\.[a-z_0-9]+)+\b")

labelled = []

# Each kind is matched over the whole file rather than line by line, for the reason the
# other label sources below already carry: an element whose text sits on the next line reads
# exactly like a file with nothing to find. Three banned words in a wrapped heading were
# invisible, and the label count did not move.
#
# An attribute a reader hears or reads is a label even when it is not element text, so
# placeholder and aria-label are here beside the elements. `option` is the one a first-year
# meets most often, in a list they are choosing from.
#
# Each kind carries the shape that says it is present at all, so a kind the interface does
# not use is told apart from a pattern that cannot read the kind it names. Flagging the
# first would leave this rule permanently red on an interface with no tables, which is
# worse than the narrowing it was written to catch.
MARKUP_LABELS = (
    (r"<h[1-4]\b", r"<h[1-4]\b[^>]*>([^<]+)", "a heading"),
    # The whole element, because a label's text can sit past its input child: one of the
    # seven labels put its sentence after the checkbox and the leading-text pattern read it
    # as empty. Child tags are stripped in the loop before any word is judged.
    (r"<label\b", r"(?s)<label\b[^>]*>(.*?)</label>", "a label element"),
    (r"<legend\b", r"<legend\b[^>]*>([^<]+)", "a legend"),
    (r"<button\b", r"<button\b[^>]*>([^<]+)", "a button"),
    (r"<summary\b", r"<summary\b[^>]*>([^<]+)", "a summary"),
    (r"<option\b", r"<option\b[^>]*>([^<]+)", "an option"),
    (r"<th\b", r"<th\b[^>]*>([^<]+)", "a table heading"),
    (r"\bplaceholder=", r"\bplaceholder=\"([^\"]+)\"", "a placeholder"),
    (r"\baria-label=", r"\baria-label=\"([^\"]+)\"", "an aria-label"),
)

# What counts as a label, kept apart from how each one is read. Reporting coverage as
# "N of len(MARKUP_LABELS)" moves both sides together, so narrowing the scanner shrinks its
# own denominator and still reads as full coverage. This is the list the scanner is checked
# against, the way rule 3 checks its phrases against CONVENTIONS.md rather than itself.
READER_MEETS = (
    (r"<h[1-4]\b", "a heading"),
    (r"<label\b", "a label element"),
    (r"<legend\b", "a legend"),
    (r"<button\b", "a button"),
    (r"<summary\b", "a summary"),
    (r"<option\b", "an option"),
    (r"<th\b", "a table heading"),
    (r"\bplaceholder=", "a placeholder"),
    (r"\baria-label=", "an aria-label"),
)

markup = Path("web/index.html")
markup_text = markup.read_text()

# The labels the markup does not carry, read where each one is actually written.
#
# The rail's slot titles are the registry's own words for a construct, so they are read from
# the registry rather than from the browser: `web/registry.js` builds a slot title from
# `entry.label`, and a scan of that file for a quoted title finds nothing and always will.
# This arm was aimed at the slot titles and went blind when they moved, which is the shape
# this whole check exists to catch.
#
# Matched over the whole file rather than line by line. A call wrapped across lines reads
# exactly like a file with nothing to find, and two of these were being missed that way.
LABEL_SOURCES = (
    (Path("registry/constructs.toml"), r"^label\s*=\s*\"([^\"]*)\"", "a slot title"),
    (Path("web/workspace.js"), r"\['var\(--[a-z-]+\)',\s*'([^']*)'\]", "a legend entry"),
    # Every `label` in the method files is a parameter value's label and the rail renders
    # them all, so the whole file set is a reader-facing source. A `title` is not a label
    # and is deliberately not here, ruled in `registry/constructs.toml`'s header: a title
    # states what the quantity or the rule is in the field's own words, which is what a
    # reader matching it against a paper needs, and the label says the same thing in the
    # audience's words. Held to this rule as well, a title would be the label written twice.
    # What makes that exemption safe rather than assumed is checked below: every construct
    # declares a label, and no reader resolves a title ahead of one.
    *(
        (path, r"^label\s*=\s*\"([^\"]*)\"", "a value label the rail renders")
        for path in sorted(Path("registry/methods").glob("*.toml"))
        # A file declaring no parameter values legitimately holds no labels; one that
        # declares values and yields no labels is the blindness the loop below flags.
        if "[[method.parameter.value]]" in path.read_text()
    ),
)

# Every notice heading in the browser, found by what it is rather than by a list of files
# that would go stale the first time somebody writes one somewhere new.
NOTICE_HEADING = r"\bnotice\(\s*'[a-z]+',\s*'([^']*)'"
notice_sources = sorted(path for path in Path("web").glob("*.js"))

blind_patterns = []

# Per kind rather than per rule. One surviving witness in one category vouches for
# categories that are no longer scanned: narrowing this set from five kinds to one dropped
# eleven labels and still passed, because the control string is a heading and headings
# stayed. A list a guard iterates is a denominator.
markup_kinds_read = 0
for present, pattern, what in MARKUP_LABELS:
    found = list(re.finditer(pattern, markup_text))
    if found:
        markup_kinds_read += 1
    elif re.search(present, markup_text):
        blind_patterns.append(f"{markup} for {what}, which the file contains")
    for match in found:
        # A no-op for every kind whose capture holds no markup; for the block-captured
        # kinds it is what keeps a child's attributes from being judged as label words.
        spoken = re.sub(r"<[^>]*>", " ", match.group(1)).strip()
        if not spoken:
            continue
        number = markup_text.count("\n", 0, match.start()) + 1
        labelled.append((f"{markup}:{number}", spoken))

# A kind the reader meets, present in the file, that no scanner pattern claims. Dropping a
# pattern is what this catches, and it cannot be hidden by dropping the pattern's own
# presence probe alongside it.
scanned_probes = {probe for probe, _, _ in MARKUP_LABELS}
for probe, what in READER_MEETS:
    if probe not in scanned_probes and re.search(probe, markup_text):
        blind_patterns.append(
            f"{markup} for {what}, which the reader meets and no pattern here reads"
        )

for source, pattern, what in LABEL_SOURCES:
    if not source.exists():
        blind_patterns.append(f"{source} for {what}, which is not on disk")
        continue
    text = source.read_text()
    found = list(re.finditer(pattern, text, re.M))
    if not found:
        blind_patterns.append(f"{source} for {what}")
    for match in found:
        if not match.group(1).strip():
            continue
        number = text.count("\n", 0, match.start()) + 1
        labelled.append((f"{source}:{number}", match.group(1).strip()))

notices = 0
for source in notice_sources:
    text = source.read_text()
    for match in re.finditer(NOTICE_HEADING, text):
        if not match.group(1).strip():
            continue
        notices += 1
        number = text.count("\n", 0, match.start()) + 1
        labelled.append((f"{source}:{number}", match.group(1).strip()))
if not notices:
    blind_patterns.append(f"{len(notice_sources)} files in web/ for a notice heading")

vocabulary_failures = 0
for where, text in labelled:
    hit = banned.search(identifier.sub("", text))
    if hit:
        vocabulary_failures += 1
        report(where, f'"{text}" uses "{hit.group(0)}", which appears in 0 of 6 teaching documents')

# A pattern that silently matches nothing reads exactly like a file with nothing to find, so
# every pattern has to see something and the scan has to be caught holding a string that is
# known to be on screen.
#
# One witness per source family, and each is located by anchoring on an element id or an
# entry id rather than by the sweep the scanner runs, so a scanner narrowed to fewer kinds
# or pointed at the wrong file fails here. Located rather than written out: a literal copied
# into this file goes stale the first time somebody edits the copy, and the control then
# asserts a sentence the interface has stopped saying.
WITNESSES = (
    (markup, r'id="stage-empty".*?<h2[^>]*>([^<]+)</h2>', "the heading on the first screen"),
    (markup, r'<button[^>]*\bid="load-demo"[^>]*>([^<]+)</button>', "the button that opens a demonstration trial"),
    (Path("registry/constructs.toml"), r'id\s*=\s*"movement_onset".*?^label\s*=\s*"([^"]+)"',
     "the slot title for the start of the jump"),
)
read_texts = {text for _, text in labelled}
for source, pattern, what in WITNESSES:
    if not source.exists():
        report("rule 2", f"cannot read {source}, which carries {what}")
        continue
    found = re.search(pattern, source.read_text(), re.S | re.M)
    if not found:
        report("rule 2", f"could not find {what} in {source}, so this control cannot vouch for anything")
    elif found.group(1).strip() not in read_texts:
        report("rule 2", f"read {len(labelled)} labels and not {what}, "
                         f'"{found.group(1).strip()}" in {source}, so the scan is narrower than it reports')

for blind in blind_patterns:
    report("rule 2", f"read nothing from {blind}, so it is not checking what it claims to")

# ---------------------------------------- what makes the title exemption safe rather than luck
#
# A title is exempt because a reader never meets one: every construct declares a label, and
# every resolution reads the label first, `entry?.label || entry?.title || construct`. Six
# construct titles carry a word this rule bans and would become live failures the day either
# half stopped holding, so both halves are checked rather than assumed.
#
# The two halves fail differently and neither implies the other. A construct with no label
# reaches its title through a fallback that is written correctly; a reader written the other
# way round reaches it past a label that is present.

constructs = Path("registry/constructs.toml")
if not constructs.exists():
    report("rule 2", f"cannot read {constructs}, which is where the titles are")
else:
    text = constructs.read_text()
    rows = [block for block in text.split("[[construct]]")[1:]]
    if not rows:
        report("rule 2", f"read no construct from {constructs}, so the title exemption is unchecked")
    titled = [row for row in rows if re.search(r'^title\s*=\s*"[^"]', row, re.M)]
    unlabelled = [
        re.search(r'^id\s*=\s*"([^"]+)"', row, re.M).group(1)
        for row in rows
        if re.search(r'^id\s*=\s*"([^"]+)"', row, re.M)
        and not re.search(r'^label\s*=\s*"[^"]', row, re.M)
    ]
    # The title population is the control: a parse that read no title would report every
    # construct as safely labelled while seeing nothing it was aimed at.
    if len(titled) < len(rows):
        report("rule 2", f"read a title from {len(titled)} of {len(rows)} constructs, so this "
                         f"is not reading the field the exemption is about")
    if unlabelled:
        report("rule 2", f"{len(unlabelled)} of {len(rows)} constructs declare no label, so a "
                         f"reader falls through to a title this rule does not check: {unlabelled}")

# `slot?.title` is the rail's already-resolved label rather than a construct's title, so the
# pattern anchors on the registry entry the resolution reads from.
title_first = re.compile(r"\bentry\??\.title\b[^;\n]*\|\|[^;\n]*\bentry\??\.label\b")
resolvers = sorted(path for path in Path("web").glob("*.js"))
resolves_a_construct = [path for path in resolvers if "entry?.label" in path.read_text()]
if not resolves_a_construct:
    report("rule 2", f"no file in web/ resolves a construct's words, across {len(resolvers)} "
                     f"scanned, so the title exemption is unchecked on the surface a reader uses")
for path in resolves_a_construct:
    source = path.read_text()
    found = title_first.search(source)
    if found:
        number = source.count("\n", 0, found.start()) + 1
        report(f"{path}:{number}", "resolves a construct's title ahead of its label, and six "
                                   "titles carry a word this rule bans in a label")

if not blind_patterns and not vocabulary_failures and not any(f.startswith("rule 2 ") for f in failures):
    print(f"pass  rule 2, {len(labelled)} labels the audience reads use the words the "
          f"audience uses, across {markup_kinds_read} of {len(READER_MEETS)} markup kinds "
          f"the reader meets, {len(LABEL_SOURCES)} further label sources and "
          f"{notices} notice headings, with {len(WITNESSES)} witnesses found by id. "
          f"A title is not a label: all {len(rows)} constructs declare one of each, and the "
          f"{len(resolves_a_construct)} files in web/ that resolve a construct's words read "
          f"the label first")


# --------------------------------------------------- 3. strings about the software

ABOUT_THE_SOFTWARE = (
    "in this build", "not implemented yet", "coming soon", "build default",
    "available here", "listed disabled", "generated in this tab",
)
about = re.compile("|".join(re.escape(phrase) for phrase in ABOUT_THE_SOFTWARE), re.IGNORECASE)

# A sample this rule owns, carrying one banned phrase in each position the scan tells apart.
# An extractor that stopped reading either position, or stopped telling them apart, fails
# here rather than on the interface, and the expectation is written out rather than computed
# from the sample, which would agree with itself whatever the extractor did.
#
# The pattern literal on the last line is the trap that makes a quote-hunting scan run to the
# end of the file: a scan that opens a string on it reads the rest of the sample as one
# literal and the comment phrases turn up in the wrong bucket.
CONTROL_SCRIPT = (
    "// a note that says coming soon\n"
    "const offered = 'These rules are not implemented yet';\n"
    "const split = 'in this ' + 'build';\n"
    "/* a block that says generated in this tab */\n"
    "const fine = `Choose a file`;\n"
    "const quoted = /['\"]/;\n"
)
CONTROL_MARKUP = (
    "<!-- a note that says coming soon -->\n"
    "<p>These rules are not implemented yet</p>\n"
    "<input placeholder=\"build default\">\n"
)
CONTROL_STYLESHEET = (
    "/* a block that says generated in this tab */\n"
    ".empty::after { content: 'coming soon'; }\n"
)
CONTROLS = (
    (Path("control.js"), CONTROL_SCRIPT,
     ["in this build", "not implemented yet"], ["coming soon", "generated in this tab"]),
    (Path("control.html"), CONTROL_MARKUP, ["build default", "not implemented yet"], ["coming soon"]),
    (Path("control.css"), CONTROL_STYLESHEET, ["coming soon"], ["generated in this tab"]),
)

control_failures = 0
for path, sample, expect_read, expect_in_comments in CONTROLS:
    strings, comments = split_source(path, sample)
    found_read = sorted({phrase.lower() for _, text in strings for phrase in about.findall(text)})
    found_comments = sorted({phrase.lower() for _, text in comments for phrase in about.findall(text)})
    if found_read != sorted(expect_read):
        control_failures += 1
        report("rule 3", f"the {path.suffix} scan read {found_read} from its own control sample "
                         f"where {sorted(expect_read)} is written into it, so what it reports "
                         f"about web/ cannot be believed")
    if found_comments != sorted(expect_in_comments):
        control_failures += 1
        report("rule 3", f"the {path.suffix} scan read {found_comments} from its own control "
                         f"sample's comments where {sorted(expect_in_comments)} is written into "
                         f"it, so what it reports about web/ cannot be believed")

reader_hits = 0
comment_hits = 0
strings_read = 0
comments_read = 0
scanned = 0
read = set()
for path in sorted(Path("web").rglob("*")):
    if path.is_dir() or "pkg" in path.parts or path.suffix not in {".html", ".js", ".css"}:
        continue
    scanned += 1
    read.add(path.as_posix())
    strings, comments = split_source(path, path.read_text())
    strings_read += len(strings)
    comments_read += len(comments)
    for number, text in strings:
        hit = about.search(text)
        if hit:
            reader_hits += 1
            report(f"{path}:{number}", f'"{hit.group(0)}" describes the state of the software')
    for number, text in comments:
        hit = about.search(text)
        if hit:
            comment_hits += 1
            report(f"{path}:{number}", f'"{hit.group(0)}" describes the state of the software, in a '
                                       f"comment, which CONVENTIONS.md section 5 reaches as well as a string")

# The list is checked against CONVENTIONS.md section 5 in both directions, because each
# direction catches a different failure: a phrase here that section 5 does not ban is a
# check enforcing something nobody agreed, and a phrase section 5 bans that nothing here
# matches is a ban with no enforcement. A probe built out of this tuple would match itself
# whatever it said, which is a control that cannot fail.
# The paragraph rather than the section: section 5 also quotes a sentence as an illustration
# of the class, and a parse that swept the whole section would read the illustration as an
# item on the list. Whitespace is normalised inside the paragraph because the list wraps
# across lines, and a pattern that does not tolerate the wrap undercounts silently.
paragraphs = Path("CONVENTIONS.md").read_text().split("\n\n")
listed = [" ".join(block.split()) for block in paragraphs if "Banned outright:" in block]
banned_in_conventions = [phrase for block in listed for phrase in re.findall(r'"([^"]+)"', block)]

if not banned_in_conventions:
    control_failures += 1
    report("rule 3", "CONVENTIONS.md yielded no banned list, so this rule is checking itself")
unagreed = [phrase for phrase in ABOUT_THE_SOFTWARE
            if not any(phrase.lower() in quoted.lower() for quoted in banned_in_conventions)]
if unagreed:
    control_failures += 1
    report("rule 3", f"{len(unagreed)} of {len(ABOUT_THE_SOFTWARE)} phrases are not in CONVENTIONS.md "
                     f"section 5's banned list, so this check enforces something nobody agreed: {unagreed}")

unenforced = [quoted for quoted in banned_in_conventions
              if not about.search(quoted)]
if unenforced:
    control_failures += 1
    report("rule 3", f"{len(unenforced)} of {len(banned_in_conventions)} phrases CONVENTIONS.md section 5 "
                     f"bans are matched by nothing here: {unenforced}")

# The scan reaching the file the copy is written in, named rather than counted, so narrowing
# the glob cannot read as an interface that stopped saying these things.
CARRIES_THE_COPY = "web/index.html"
if CARRIES_THE_COPY not in read:
    control_failures += 1
    report("rule 3", f"read {scanned} files and not {CARRIES_THE_COPY}, which is where the copy is")

if not reader_hits and not comment_hits and not control_failures:
    print(f"pass  rule 3, none of the {strings_read} strings a reader meets or the {comments_read} "
          f"comments across {scanned} files in web/ describes the state of the software "
          f"({len(ABOUT_THE_SOFTWARE)} phrases, each on CONVENTIONS.md section 5's list of "
          f"{len(banned_in_conventions)} and each matching it, and each found again in "
          f"{len(CONTROLS)} control samples written to carry them)")


for failure in failures:
    print(f"FAIL  {failure}")
if failures:
    print(f"\n{len(failures)} failures")
    sys.exit(1)
print("\nthree rules, no failures")
CHECKS
