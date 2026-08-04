#!/usr/bin/env python3
"""Compare every listed request, computed on every surface, against one committed record.

Against a committed baseline rather than between the surfaces: several surfaces wrong the
same way agree with each other perfectly, and a committed file makes every change a diff a
reviewer reads.

Over a population of requests rather than one, because a field can only be compared where
some request puts a value in it. The gate asked a single clean trial and reported agreement
about `refusals`, `signals` and `warnings` while all three were empty in every answer: four
surfaces agreeing about a shape and about no value. `EMPTY_ON_EVERY_REQUEST` is where a field
the whole population leaves empty has to be named, and a field left empty and unnamed makes
this gate refuse.

What this does not prove, stated because a green here is easy to over-read. `serde_json`
writes the shortest string that round-trips, so the wire text is identical whether or not a
build carries `float_roundtrip`, and only the double a caller ends up holding differs. Bytes
therefore cannot see that defect at all. The reading side is asserted inside each runtime
against a second reader in that same runtime, because five surfaces sharing one wrong parser
agree with each other: `crates/plateforce-cli/tests/result_parity.rs` for Rust and
`bindings/r/tests/testthat/test-doubles.R` for R.

How far the comparison reaches is measured from the surfaces rather than listed. Every field
every surface publishes is compared here, asserted another way in `ASSERTED_ANOTHER_WAY`, or
carried by some surfaces and not others and recorded in `SURFACES_THAT_DIFFER`. A field in
none of the three makes this gate refuse, so it cannot print that four surfaces computed the
result while a field of that result went unread. `scripts/prove-parity-coverage-refuses.py`
is where each of those refusals is shown to fire.

Measured from the surfaces and then checked against the document, because the two are not the
same universe. Those three registers are keyed by what the answers hold, and `serde` drops a
field the whole population leaves empty, so such a field is compared by nobody, asserted by
nobody and missing from nobody: every count stays true and every one of them is narrower than
the result. The fields are therefore read off the struct that declares them, and one reaching
no wire at all is named in `NEVER_ON_THE_WIRE` with the request that would fill it.

Over two kinds of question, because a field's reach is a fact about the question and not
about the field. An analysed document carries `plateforce_version` on the two surfaces that
assemble it; a swept one carries it on every surface that answers, so one register keyed by
field alone would have to state one of the two and be wrong about the other. Both registers
are keyed by kind, and a request carrying a `sweep` block is a sweep.
"""

import json
import pathlib
import re
import sys
from typing import NamedTuple

# A floor rather than the exact count, which would fail every time a metric is added. It is
# here because a comparison that agreed on an empty document would report success.
#
# Measured rather than picked, by the count this module prints: the four committed requests
# carry 22, 22, 9 and 17 numbers, 70 between them. The floor sits well below that so a metric
# coming or going is not an alarm, and far enough above zero to catch a population that
# collapsed. It is a population figure because a request whose subject is a partial result
# carries fewer numbers honestly: the interrupted recording carries 9, and holding it to a
# per-request floor of 10 would redden the gate for answering the question it was written to
# ask. The per-request floor is instead one number, below, which a document agreeing about
# nothing cannot meet.
NUMBERS_THE_POPULATION_CARRIES = 10

# Every surface the gate speaks for, and every request it speaks about, read from the
# manifests the harness reads.
SURFACE_MANIFEST = pathlib.Path(__file__).with_name("result-parity-surfaces.txt")
REQUEST_MANIFEST = pathlib.Path(__file__).with_name("result-parity-requests.txt")
ROOT = pathlib.Path(__file__).parent.parent

# Where a request file lives and how it is named, so a file wired to nothing is something
# this gate finds rather than something a reader has to go looking for. Two such files sat in
# this repository for a day, each asking a question no surface was ever asked.
GOLDEN = ROOT / "tests" / "golden"
REQUEST_GLOB = "result-parity-request*.json"
BASELINE_GLOB = "result-parity*.json"


def canonical(value):
    """One writer for every side of every comparison, applied to each surface's own bytes.

    A surface cannot pass by formatting differently in a way the comparison forgives, and a
    surface cannot fail for putting a space somewhere.
    """
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def result_in(envelope, name):
    if not isinstance(envelope, dict) or "ok" not in envelope:
        raise SystemExit(f"plateforce: {name} returned no result: {canonical(envelope)[:400]}")
    return envelope["ok"]


class Request(NamedTuple):
    """One row of the request manifest.

    `baseline` is the record this request is held to. `equals` names another row when the
    two share that record, which is how an invariant between two requests is stated: the
    sentinel request declares a convention matching 157 real samples of the trial the quiet
    request reads, and the claim is that declaring it moves no number. Two identical
    committed copies would say the same thing until somebody regenerated one of them.

    `surfaces` is who is asked, read off the row rather than assumed to be everybody.
    `kind` is what is asked, read off the request file, because the shape of the question
    is the request's own statement rather than a second one a manifest could contradict.
    """

    name: str
    request: str
    baseline: str
    equals: str
    surfaces: frozenset
    kind: str


# The two kinds of question this gate asks. A request carrying a `sweep` block asks how far
# the number moves across a slot's alternatives; one without it asks what the analysis
# reports. Each surface's arm makes this same test, in its own language, on the same key.
ANALYSED = "analysed"
SWEPT = "sweep"


def kind_of(request_path):
    """Which question a request file asks, taken from the file that asks it."""
    path = GOLDEN / pathlib.Path(request_path).name
    if not path.exists():
        return ANALYSED
    with open(path, encoding="utf-8") as handle:
        return SWEPT if SWEPT in json.load(handle) else ANALYSED


def requests_named_in_manifest():
    """Every request this gate speaks about, read from the file the harness reads.

    The manifest is the only place a request is named. A file in tests/golden that no row
    names is a question nobody asks, which `request_manifest_faults` refuses rather than
    passes over.
    """
    rows = []
    for line in REQUEST_MANIFEST.read_text(encoding="utf-8").splitlines():
        # Not stripped whole, because a row whose third or fourth cell is empty ends in a tab
        # and stripping it would read as a row of fewer cells. A request held to nothing, or
        # asked of nobody, is a refusal with a sentence of its own, and a parse error in its
        # place says the wrong thing about what somebody forgot.
        row = line.rstrip("\n")
        if not row.strip() or row.lstrip().startswith("#"):
            continue
        cells = [cell.strip() for cell in row.split("\t")]
        if len(cells) != 4:
            raise SystemExit(
                f"plateforce: {REQUEST_MANIFEST.name} row {row!r} does not carry a name, a "
                "request file, the record it is held to and the surfaces it is asked of"
            )
        name, request, held, asked = cells
        surfaces = frozenset(part for part in asked.split(",") if part)
        kind = kind_of(request)
        if held.startswith("="):
            rows.append(Request(name, request, "", held[1:], surfaces, kind))
        else:
            rows.append(Request(name, request, held, "", surfaces, kind))
    if not rows:
        raise SystemExit(f"plateforce: {REQUEST_MANIFEST} names no request")
    return rows


def baseline_of(rows, row):
    """The committed record a row is held to, following one `=` hop and no more."""
    if not row.equals:
        return row.baseline
    return {other.name: other for other in rows}[row.equals].baseline


def request_manifest_faults(rows, writing=False):
    """Every request file is asked, every asked request is held to a record, or this refuses.

    A gate that quietly compares fewer things than it claims is the defect this whole file
    exists against, and a request added without a baseline is the cheapest way back into it.
    So a missing record is a refusal and never a skip, in both directions: a row naming a
    record that is not there, and a file on disk that no row names.

    `writing` drops the one fault a regeneration is about to fix, a named baseline that is
    not on disk yet. Every other refusal stands: a run that would write a record for a
    request file nobody can find, or leave a request file unasked, is writing a standard for
    a population it has not established.
    """
    faults = []
    named = {row.name: row for row in rows}
    if len(named) != len(rows):
        faults.append("two rows of the request manifest carry one name")

    # Who is asked what, in both directions. A row that quietly dropped a surface would
    # report agreement between the ones left, and a surface no row asks is a row of
    # `result-parity-surfaces.txt` this gate speaks for and never exercises.
    listed = surfaces_named_in_manifest()
    asked_somewhere = set()
    for row in rows:
        asked_somewhere |= row.surfaces
        unknown = sorted(row.surfaces - listed)
        if unknown:
            faults.append(
                f"{row.name} is asked of {unknown}, and {SURFACE_MANIFEST.name} names no such "
                f"surface, so nothing would answer it"
            )
        if len(row.surfaces) < 2:
            faults.append(
                f"{row.name} is asked of {sorted(row.surfaces)}, and a request one surface "
                "answers is a surface agreeing with itself. Every request owes two or more"
            )
        for surface in sorted(listed - row.surfaces):
            if surface not in SURFACES_NOT_ASKED.get(row.kind, {}):
                faults.append(
                    f"{row.name} is not asked of {surface} and nothing here says why. A "
                    "surface left off a request is a question that surface's readers cannot "
                    "put to it, so it is stated with the work that would change it"
                )
    for surface in sorted(listed - asked_somewhere):
        faults.append(
            f"{SURFACE_MANIFEST.name} names {surface} and no request asks it, so this gate "
            "speaks for a surface it never exercises"
        )

    # An entry whose account of itself rests on another kind of request being asked. Without
    # this the sentence outlives the request: the register would go on saying the sweep is
    # compared elsewhere after the row that compared it had gone, which is the shape the
    # `spread` entry was already in when it said the other surfaces expose a call of their own.
    kinds = {row.kind for row in rows}
    for kind, register in sorted(SURFACES_THAT_DIFFER.items()):
        for field, declared in sorted(register.items()):
            if declared.answered_by and declared.answered_by not in kinds:
                faults.append(
                    f"{field} rests on a {declared.answered_by} request and this population "
                    f"holds none, so its account of itself is a sentence nothing measures. "
                    f"Discharged by {declared.discharged_by}"
                )

    for kind, unasked in sorted(SURFACES_NOT_ASKED.items()):
        for surface, declared in sorted(unasked.items()):
            asked_anyway = sorted(
                row.name for row in rows if row.kind == kind and surface in row.surfaces
            )
            if asked_anyway:
                faults.append(
                    f"{surface} is recorded as unable to answer a {kind} request and "
                    f"{asked_anyway} ask it, so the entry is out of date and the surface "
                    f"answers for real. Discharged by {declared.discharged_by}"
                )
            elif not any(row.kind == kind for row in rows):
                faults.append(
                    f"{surface} is recorded as unable to answer a {kind} request and this "
                    f"population holds no {kind} request at all, which reads as coverage and "
                    f"covers nothing: {declared.reason}"
                )

    for row in rows:
        if not (GOLDEN / pathlib.Path(row.request).name).exists():
            faults.append(f"{row.name} names the request {row.request} and there is no such file")
        if row.equals:
            if row.equals not in named:
                faults.append(
                    f"{row.name} is declared equal to {row.equals} and no row is named that, so "
                    "it is held to nothing"
                )
            elif row.equals == row.name:
                faults.append(f"{row.name} is declared equal to itself, which asserts nothing")
            elif named[row.equals].equals:
                faults.append(
                    f"{row.name} is declared equal to {row.equals}, which is itself declared "
                    f"equal to {named[row.equals].equals}. One hop, so a population cannot end "
                    "up pinned to nothing through a chain nobody read to the end"
                )
        elif not row.baseline:
            faults.append(
                f"{row.name} is held to no record at all. Every request owes a committed "
                "baseline or the name of a row it answers identically"
            )
        elif not (ROOT / row.baseline).exists() and not writing:
            faults.append(
                f"{row.name} is held to {row.baseline} and there is no such record. Write it "
                "with scripts/result-parity.sh --write and audit the diff before committing it"
            )

    asked = {pathlib.Path(row.request).name for row in rows}
    for found in sorted(GOLDEN.glob(REQUEST_GLOB)):
        if found.name not in asked:
            faults.append(
                f"{found.name} asks a question and no row of {REQUEST_MANIFEST.name} names it, "
                "so no surface is ever asked it"
            )

    # Resolved from the rows that carry a record of their own, so a row whose `=` target is
    # missing is reported once, above, rather than crashing this scan on its way past.
    held = {pathlib.Path(row.baseline).name for row in rows if row.baseline}
    for found in sorted(GOLDEN.glob(BASELINE_GLOB)):
        if found.name in asked or found.name in held:
            continue
        faults.append(
            f"{found.name} is a committed record no row is held to, so it is a standard "
            "nothing is measured against"
        )
    return faults


def answers_from(directory, row):
    """One request's answers, one file per surface, named by the harness that collected them.

    Over the surfaces the row is asked of rather than over every listed surface, and the row
    is what the harness read to decide who to run, so a surface missing here is one that was
    asked and did not answer.
    """
    answers = {}
    for surface in sorted(row.surfaces):
        path = pathlib.Path(directory) / f"{row.name}.{surface}.json"
        if not path.exists():
            raise SystemExit(
                f"plateforce: {surface} left no answer to {row.name}, so this run holds "
                "fewer surfaces than that request is asked of"
            )
        with open(path, encoding="utf-8") as handle:
            answers[surface] = result_in(json.load(handle), f"{surface} on {row.name}")
    return answers


def differing_paths(committed, reported, path=""):
    """Name the field rather than a byte offset, so a failure says what moved."""
    if isinstance(committed, dict) and isinstance(reported, dict):
        found = []
        for key in sorted(set(committed) | set(reported)):
            here = f"{path}.{key}" if path else key
            if key not in committed:
                found.append(f"{here}: only the surface has it")
            elif key not in reported:
                found.append(f"{here}: only the committed result has it")
            else:
                found += differing_paths(committed[key], reported[key], here)
        return found
    if isinstance(committed, list) and isinstance(reported, list):
        if len(committed) != len(reported):
            return [f"{path}: committed holds {len(committed)}, the surface holds {len(reported)}"]
        found = []
        for index, (was, now) in enumerate(zip(committed, reported)):
            found += differing_paths(was, now, f"{path}[{index}]")
        return found
    if committed != reported:
        return [f"{path}: committed {committed!r}, the surface reports {reported!r}"]
    return []


def projected(answer, fields, name):
    """The engine's own response fields, which every surface carries.

    Each surface wraps them in an envelope of its own and adds what only it knows, a file
    path here, a description block there. Comparing whole envelopes would report those as
    divergence and say nothing about the numbers, so the comparison is over the fields the
    engine produced and a surface that stops carrying one fails by name.
    """
    missing = [field for field in fields if field not in answer]
    if missing:
        raise SystemExit(f"plateforce: {name} carries no {missing}, which every surface reports")
    return {field: answer[field] for field in fields}


def compared_fields_in(baseline_path):
    try:
        with open(baseline_path, encoding="utf-8") as handle:
            fields = json.load(handle)["compared_fields"]
    except (OSError, KeyError, json.JSONDecodeError):
        raise SystemExit(
            f"plateforce: {baseline_path} names no compared_fields, and which fields every "
            "surface owes is a decision rather than a measurement"
        )
    if not fields:
        raise SystemExit("plateforce: compared_fields is empty, so this gate asserts nothing")
    return fields


def surfaces_named_in_manifest():
    """Every surface this gate speaks for, read from the file the harness reads.

    A run over some of the surfaces would report agreement between the ones it asked and read
    as agreement between the four the manifest names, and would widen the intersection below
    at the same time. This gate speaks for every listed surface or it does not speak.
    """
    named = []
    for line in SURFACE_MANIFEST.read_text(encoding="utf-8").splitlines():
        row = line.strip()
        if row and not row.startswith("#"):
            named.append(row.split("\t")[0].strip())
    if not named:
        raise SystemExit(f"plateforce: {SURFACE_MANIFEST} names no surface")
    return set(named)


def fields_every_surface_publishes(answers):
    """The whole of what this comparison can reach, measured rather than declared.

    `projected` raises on a field a surface does not carry, so a field one surface publishes
    and another does not cannot be held to a single committed value at all. This set is
    therefore not a judgement about which fields are worth comparing. It is the reach of the
    mechanism, and every field in it owes either a comparison or an assertion of its own.
    """
    return set.intersection(*(set(answer) for answer in answers.values()))


def surfaces_publishing(answers, field):
    return frozenset(name for name, answer in answers.items() if field in answer)


# Where the two documents this gate compares are declared, and the struct each kind of question
# is answered in.
DOCUMENT_SOURCE = ROOT / "crates" / "plateforce-analysis" / "src"
DOCUMENT_OF_KIND = {ANALYSED: "ResultDocument", SWEPT: "SpreadDocument"}

_STRUCT = re.compile(r"^pub struct (\w+)\b")
_FIELD = re.compile(r"^\s{4}pub ([a-z_][a-z_0-9]*)\s*:\s*(.+?),?\s*$")
_RENAMED = re.compile(r'rename\s*=\s*"([^"]+)"')


def keys_a_document_declares(struct, seen=None):
    """Every key the named document writes on the wire, read off the struct that declares it.

    The reach measured from the answers is the intersection of what the surfaces published, and
    a field absent from all four is outside it: `serde` drops a field the whole population
    leaves empty, so it appears in no answer, matches no register, and the gate goes on
    reporting that every field was accounted for. `plate_profile` is exactly that shape, a
    provenance field on the document and on nobody's answer.

    So the universe is the declaration rather than the answers. A field carrying
    `#[serde(flatten)]` contributes the keys of the type it names instead of its own, which is
    how `SpreadDocument` puts fifteen of the sweep's keys at the top level.
    """
    seen = seen or set()
    if struct in seen:
        raise SystemExit(f"plateforce: {struct} flattens into itself")
    for path in sorted(DOCUMENT_SOURCE.rglob("*.rs")):
        lines = path.read_text(encoding="utf-8").splitlines()
        for at, line in enumerate(lines):
            found = _STRUCT.match(line)
            if not found or found.group(1) != struct or not line.rstrip().endswith("{"):
                continue
            return _keys_from(lines, at + 1, struct, seen | {struct})
    raise SystemExit(
        f"plateforce: no struct named {struct} under {DOCUMENT_SOURCE}, so the fields this gate "
        "should account for could not be read at all"
    )


def _keys_from(lines, at, struct, seen):
    keys = set()
    attributes = []
    for line in lines[at:]:
        if line.startswith("}"):
            break
        stripped = line.strip()
        if stripped.startswith("#["):
            attributes.append(stripped)
            continue
        found = _FIELD.match(line)
        if not found:
            if stripped and not stripped.startswith("//"):
                attributes = []
            continue
        name, kind = found.group(1), found.group(2)
        written = "".join(attributes)
        attributes = []
        if "flatten" in written:
            keys |= keys_a_document_declares(kind.strip(), seen)
            continue
        renamed = _RENAMED.search(written)
        keys.add(renamed.group(1) if renamed else name)
    if not keys:
        raise SystemExit(f"plateforce: {struct} declares no field this gate can read")
    return keys


# A field every surface publishes that this gate does not compare against the committed
# document, with the assertion that covers it instead. Naming a field here is not permission
# to stop looking at it: each entry owes a function, `coverage_faults` runs it, and an entry
# without one cannot be written. Widening `compared_fields` is the ordinary answer and this
# is the exception, so an entry states why a committed value would be the wrong instrument.
#
# Keyed by the kind of question, because a field reaching every surface on one kind and two
# surfaces on another is one field in two states and a single entry could only state one.
REGISTRY_ASSERTIONS = {
    "registry_digest": (
        "the digest moves with every registry edit, so a committed value would redden this "
        "gate for a reason that is not about parity, while two surfaces reading different "
        "registries is exactly parity",
        lambda answers: surfaces_read_one_registry(answers),
    ),
    "registry_declared_version": (
        "the revision the registry names about itself moves whenever registry/VERSION is "
        "edited, which is a registry event and not a parity one, and a committed copy of it "
        "is one more provenance figure nobody checks. Two surfaces disagreeing about what "
        "the registry called itself is exactly parity, so that is what is asserted",
        lambda answers: surfaces_name_one_revision(answers),
    ),
}

ASSERTED_ANOTHER_WAY = {
    ANALYSED: {
        **REGISTRY_ASSERTIONS,
        "descriptions": (
            "an account names the registry behind the number it describes, so a committed "
            "copy of one moves with every registry data edit, which is how a method is added "
            "here and is not a parity event. It is the same property the digest itself is "
            "asserted for one entry above, reaching this field through the prose. Two "
            "surfaces giving one number two accounts is exactly parity, so that is what is "
            "asked",
            lambda answers: surfaces_write_one_account_of_each_number(answers),
        ),
    },
    SWEPT: {
        **REGISTRY_ASSERTIONS,
        "plateforce_version": (
            "the build that produced the numbers moves with every release, so a committed "
            "value would redden this gate on a version bump, which is not a parity event. "
            "Every surface that answers a sweep carries it, unlike an analysed document "
            "where two do, so what is asserted is that they name one build",
            lambda answers: surfaces_name_one_build(answers),
        ),
    },
}

class Divergence(NamedTuple):
    """One field some surfaces publish and others do not, as measured rather than as hoped.

    `carriers_agree` is the state the surfaces carrying it are in: True when they report the
    same value, False when they do not, and None when only one carries it, because a
    population of one agrees with itself and recording that as agreement would claim a
    cross-surface agreement nobody tested. It is pinned to measurement, so a divergence being
    repaired reddens this gate exactly as loudly as one appearing. The repair is meant to end
    with the entry moving out of this register.

    `discharged_by` names the work that retires the entry, so an entry cannot quietly become
    permanent by nobody remembering what it was waiting for.

    `answered_by` names a kind of request that has to be in the population for the entry's
    account of itself to hold. A discharge saying the other surfaces compute this elsewhere
    is prose until something asks them, and the sentence would outlive the request that made
    it true. Empty where the entry claims no such thing.
    """

    carried_by: frozenset
    carriers_agree: bool
    discharged_by: str
    reason: str
    answered_by: str = ""


class NotAsked(NamedTuple):
    """One surface a kind of request is not put to, and what stands in the way.

    Checked against measurement on every run, in both directions: a surface named here that
    does answer reddens this gate, and a surface left off a row and named nowhere reddens it
    too. So the population cannot narrow by a surface quietly dropping off a request, which
    is the same defect as a field quietly dropping out of a comparison.

    Two things put a surface here and the reason says which. Its entry point cannot state the
    question at all, or it answers a document no single record can hold alongside the others.
    Neither is the surface computing the wrong numbers, and an entry that stopped being either
    is a row waiting to be widened.
    """

    discharged_by: str
    reason: str


SURFACES_NOT_ASKED = {
    SWEPT: {
        "r": NotAsked(
            "a `pf_spread` that takes several slots, which Python's `slot` already does and "
            "refuses beside `parameter` and `method_ids` for the same reason",
            "`pf_spread` builds one axis per call, so the sweep the terminal computes over "
            "three landmark constructs cannot be written as one R call. R sweeps one slot and "
            "answers a narrower question than the record this request is held to",
        ),
        # The browser was here, asked of nothing because it reported the same 75 combinations
        # in the order it ranks rules for a reader. `spread::run` now orders `variants` by the
        # binding table whatever order the caller listed the rules in, so one record holds the
        # tab and the terminal together and the tab is on the sweep row.
    },
}


# A field some surfaces publish and others do not. This gate cannot hold one to a committed
# value, because there is no value on every surface to hold, so the register exists to make
# the divergence something a reader of this gate's output can see rather than something its
# silence hides.
#
# Checked against measurement in both directions on every run, on which surfaces carry it and
# on whether those surfaces agree. A field that reached a fifth surface, stopped reaching one,
# or started disagreeing with itself has changed what a result carries, and a line here still
# naming the old state is not a reason to pass over it. An entry is a divergence recorded,
# never a divergence accepted.
ANALYSED_SURFACES_THAT_DIFFER = {
    "plateforce_version": Divergence(
        frozenset({"cli", "browser"}),
        True,
        "nothing outstanding; the two surfaces that assemble `ResultDocument` agree",
        "the build that produced the numbers, carried by the two surfaces that assemble "
        "`ResultDocument`. Python and R answer for a build a caller already holds, through "
        "`plateforce.__version__` and `packageVersion`",
    ),
    # `registry_version` was here, carried by cli, browser and python, and the one entry whose
    # carriers disagreed. Discharged by wsrp/registry-pin: it now means the caller's pin on
    # every surface and nothing else, R carries it for the first time, and an unpinned run
    # writes null everywhere, so it is compared rather than recorded. The registry's own claim
    # went to `registry_declared_version`, which is asserted between the surfaces below for
    # the reason the digest is.
    "trial": Divergence(
        frozenset({"cli", "browser"}),
        True,
        "nothing outstanding; a caller who opened the trial already holds this",
        "where the trace came from and what the reader had to be told about reading it. The "
        "two surfaces handed a path know it; Python and R are handed a trial somebody else "
        "opened",
    ),
    "acquisition": Divergence(
        frozenset({"cli", "browser"}),
        True,
        "nothing outstanding; the two surfaces that assemble `ResultDocument` agree",
        "what the plate and its settings were, carried whole by the two surfaces that assemble "
        "`ResultDocument`. Python and R take the block on the trial they are handed and report "
        "`acquisition_complete`, which is compared on all four",
    ),
    # `descriptions` was here, carried by r alone, with nothing to agree about. Discharged by
    # ws/descriptions-everywhere: `descriptions_of` moved beside `chains_of`, the document
    # fills the block rather than accepting it, and the two surfaces that were passing an
    # empty map cannot. It reaches all four and is compared rather than recorded.
    "provenance": Divergence(
        frozenset({"r"}),
        None,
        "the terminal, the tab and a notebook publishing the chain their own consumers "
        "already derive. All four read one derivation now, `plateforce_analysis::chain_of`, "
        "and three of them keep it inside the process rather than writing it into the "
        "document they hand out",
        "the chain of rules behind each number, one record per metric. R's package reads it "
        "off the wire because R links the engine and cannot reach the derivation any other "
        "way; the other three hold the tree in memory and publish the numbers alone",
    ),
    # A discharge naming the shape of an API is not a comparison: a call nothing asks proves
    # nothing about the value it returns. The `sweep` request is what asks these surfaces, and
    # `answered_by` refuses this sentence if that request leaves the population.
    "spread": Divergence(
        frozenset({"cli"}),
        None,
        "the sweep request, which holds the terminal, the tab and Python to one committed "
        "record over all 21 fields of a swept document, and names in SURFACES_NOT_ASKED what "
        "stands between that record and the surface still outside it. What is left here is "
        "the nesting: the terminal reports the headline sweep inside the analysed document "
        "and nobody else does",
        "how far a number moves across a slot's defensible alternatives. The terminal sweeps "
        "with the analysis; the tab sweeps on its own schedule through a second entry point, "
        "and Python and R expose the sweep as a call of its own",
        SWEPT,
    ),
}

# A swept document names one register and it is empty, because every surface asked a sweep
# publishes every field of it. Kept rather than left out: `coverage_faults` reads this by
# kind, and a field that starts reaching some surfaces and not others has to land somewhere
# a reader of this file looks.
SWEPT_SURFACES_THAT_DIFFER = {}

SURFACES_THAT_DIFFER = {
    ANALYSED: ANALYSED_SURFACES_THAT_DIFFER,
    SWEPT: SWEPT_SURFACES_THAT_DIFFER,
}


class EmptyEverywhere(NamedTuple):
    """One compared field no request in the population puts a value in.

    Naming a field here is what makes a hollow comparison visible. The surfaces agree about
    its shape, four empty lists match perfectly, and the gate's own sentence about agreement
    would otherwise cover it without a word. `discharged_by` names the request that would
    retire the entry, so it cannot quietly become permanent.

    Checked in both directions on every run: a field named here that some request does put a
    value in reddens this gate exactly as loudly as a field left empty and named nowhere.
    """

    discharged_by: str
    reason: str


# A compared field empty in every committed record. One entry, and it is the field this gate
# cannot value from a request alone.
#
# `refusals`, `signals` and `warnings` carry values across the population: the interrupted
# recording refuses and warns, the inverted one signals and warns.
EMPTY_ON_EVERY_REQUEST = {
    "registry_version": EmptyEverywhere(
        "a request that pins a registry revision, which needs a field in the request schema "
        "and a spelling of it on all four surfaces",
        "the caller's pin on the registry, which is null on every surface when nobody pinned "
        "one. Every committed request leaves it unstated, so the four surfaces agree about an "
        "absence. wsrp/registry-pin made the field mean the pin and nothing else, and gave R "
        "the field for the first time, so a request that states one would compare a value",
    ),
}


class NeverOnTheWire(NamedTuple):
    """One field the document declares that no answer in this population carries at all.

    A field every surface leaves out is invisible to every register above this one. Those are
    keyed by what the answers hold, and `serde` drops a field the whole population leaves
    empty, so the field is not compared, not asserted, not uneven, and not missing either. The
    gate's own sentence about the fields it covered goes on being true and goes on being
    narrower than the document.

    Checked in both directions on every run: a name here the document does not declare reddens
    this gate, and so does a name here that some surface's answer carries. So the entry cannot
    outlive the gap, which is what separates it from a list that only ever grows.

    `discharged_by` names the work that retires it, which for every entry of this shape is a
    request that fills the field, because a field on the wire is a field the comparison reaches
    with no register at all.
    """

    discharged_by: str
    reason: str


# A field the document declares and no committed request puts on any wire. One entry.
#
# Keyed by the kind of question, because the two documents declare different fields. A swept
# document names an empty register: every field `SpreadDocument` declares reaches every surface
# asked, and the register is kept rather than left out so a field that stops reaching them
# lands somewhere a reader of this file looks.
NEVER_ON_THE_WIRE = {
    ANALYSED: {
        "plate_profile": NeverOnTheWire(
            "a request whose acquisition block is filled from a saved plate, which needs the "
            "plate stated in the request file and a spelling of it on the surfaces that "
            "assemble the document",
            "the saved plate the acquisition block was filled from. It is absent rather than "
            "null on purpose, because a run with no saved plate behind it has nothing to "
            "attribute, and every committed request states none. So the four surfaces agree "
            "about a field none of them writes, and what a result says about the plate it came "
            "off is the one part of its provenance this gate has never compared",
        ),
    },
    SWEPT: {},
}


def is_empty(value):
    """What counts as the surfaces agreeing about a shape rather than about a value."""
    return value in ([], {}, "", None)


def hollow_population_faults(committed_by_request, fields_by_request):
    """Every compared field is valued by some request, or it is named as valued by none.

    The gate prints that four surfaces computed the committed requests. A reader takes that
    to be about the results, so a field no request fills is either named here or it is a
    field this gate compares and has never once compared a value in.
    """
    faults = []
    valued = set()
    everywhere = set()
    for name, fields in fields_by_request.items():
        committed = committed_by_request[name]
        for field in fields:
            everywhere.add(field)
            if not is_empty(committed[field]):
                valued.add(field)

    for field in sorted(everywhere - valued - set(EMPTY_ON_EVERY_REQUEST)):
        faults.append(
            f"{field} is compared and no committed request puts a value in it, so the surfaces "
            "agree about its shape and about nothing in it. Add a request that fills it, or "
            "name it in EMPTY_ON_EVERY_REQUEST with the request that would"
        )

    for field, declared in sorted(EMPTY_ON_EVERY_REQUEST.items()):
        if field not in everywhere:
            faults.append(
                f"{field} is named as valued by no request and no request compares it at all, "
                f"which reads as coverage and covers nothing: {declared.reason}"
            )
        elif field in valued:
            filled = sorted(
                name
                for name, fields in fields_by_request.items()
                if field in fields and not is_empty(committed_by_request[name][field])
            )
            faults.append(
                f"{field} is named as valued by no request and {filled} value it, so the entry "
                f"is out of date and the field is compared for real. Discharged by "
                f"{declared.discharged_by}"
            )
    return faults


def surfaces_read_one_registry(answers):
    """Asserted between the surfaces rather than against a committed value.

    Two surfaces resolving rules out of different registries agree about nothing that matters,
    however well their numbers happen to line up.
    """
    digests = {name: answer.get("registry_digest") for name, answer in answers.items()}
    if len(set(digests.values())) > 1:
        return [f"surfaces read different registries: {digests}"]
    return []


def surfaces_name_one_revision(answers):
    """Asserted between the surfaces rather than against a committed value.

    Read alongside `surfaces_read_one_registry`, which asks the harder question. Two surfaces
    can name one revision over different bytes, because the revision lives in a `VERSION` file
    the digest's walk does not read, so this is the weaker claim and never a substitute for it.
    """
    revisions = {
        name: answer.get("registry_declared_version") for name, answer in answers.items()
    }
    if len(set(revisions.values())) > 1:
        return [f"surfaces name different registry revisions: {revisions}"]
    return []


def surfaces_write_one_account_of_each_number(answers):
    """Asserted between the surfaces rather than against a committed value.

    The strongest form the field can be held to here: byte for byte, every surface's whole
    block, so a step at a different depth or a value spelled differently fails as loudly as a
    quantity described by one surface and not another. The quantities are named rather than
    the blocks printed, because four accounts of eleven numbers is a page of prose and the
    question a reader has is which number two surfaces disagree about.

    A block empty on every surface is reported rather than passed. Four surfaces agreeing
    about no account at all is the hollow comparison this file exists against, and it reads
    exactly like agreement.
    """
    described = {name: answer.get("descriptions") or {} for name, answer in answers.items()}
    if not any(described.values()):
        return [
            "no surface describes any number on this request, so this assertion compares "
            "four empty blocks. A request whose quantities all decline states that through "
            "`refusals`; this field would be agreeing about nothing"
        ]

    quantities = sorted(set().union(*(set(block) for block in described.values())))
    faults = []
    for quantity in quantities:
        written = {name: block.get(quantity) for name, block in described.items()}
        if len({canonical(account) for account in written.values()}) > 1:
            carried = sorted(name for name, account in written.items() if account is not None)
            faults.append(
                f"surfaces give {quantity} different accounts of itself, carried by {carried}"
            )
    return faults


def surfaces_name_one_build(answers):
    """Asserted between the surfaces rather than against a committed value.

    A sweep leaving a surface on its own says which build produced it, and a reader holding
    two of them has no way to tell a version bump from two surfaces built out of step. The
    committed value would move on every release; the disagreement is what parity is about.
    """
    builds = {name: answer.get("plateforce_version") for name, answer in answers.items()}
    if len(set(builds.values())) > 1:
        return [f"surfaces name different builds: {builds}"]
    return []


def compared_fields_measured_from(answers, kind):
    """What `compared_fields` should name: everything the surfaces all publish, less the
    fields asserted another way.

    Derived rather than maintained, which is the whole point. Read out of the baseline it
    checks, this list could only ever be widened by hand, so a field added to all four
    surfaces stayed invisible to the gate until somebody noticed.
    """
    return sorted(fields_every_surface_publishes(answers) - set(ASSERTED_ANOTHER_WAY[kind]))


def coverage_faults(answers, fields, kind, asked):
    """Every field a surface publishes is compared here, asserted another way, or a declared
    divergence. A field in none of the three is a field nothing looks at.

    This gate prints that four surfaces computed one result. A reader takes that to be about
    the result, not about the six fields of it somebody listed, so the gate refuses rather
    than publish a verdict narrower than its own sentence.
    """
    faults = []
    asserted = ASSERTED_ANOTHER_WAY[kind]
    differ = SURFACES_THAT_DIFFER[kind]
    if set(answers) != set(asked):
        faults.append(
            f"this request is asked of {sorted(asked)} and this run holds {sorted(answers)}, "
            "so nothing below speaks for the surfaces it claims"
        )
        return faults

    everywhere = fields_every_surface_publishes(answers)
    somewhere = set.union(*(set(answer) for answer in answers.values()))

    unread = sorted(everywhere - set(fields) - set(asserted))
    if unread:
        faults.append(
            f"every surface publishes {unread} and nothing here looks at any of them, so "
            f"this gate would report agreement about {len(fields)} of "
            f"{len(everywhere)} fields and say four surfaces computed the result. Regenerate "
            "with scripts/result-parity.sh --write, which derives the list from the surfaces"
        )

    for field in sorted(set(fields) - everywhere):
        faults.append(
            f"compared_fields names {field} and only {sorted(surfaces_publishing(answers, field))} "
            f"publish it, so it cannot be held to one committed value"
        )

    for field in sorted(set(fields) & set(asserted)):
        faults.append(
            f"{field} is compared against the committed document and also declared asserted "
            "another way, so one of the two is wrong"
        )

    for field, (reason, assertion) in sorted(asserted.items()):
        if field not in somewhere:
            faults.append(
                f"{field} is declared asserted another way and no surface publishes it, which "
                f"reads as coverage and covers nothing: {reason}"
            )
            continue
        faults += assertion(answers)

    for field in sorted(somewhere - everywhere):
        carried = surfaces_publishing(answers, field)
        declared = differ.get(field)
        if declared is None:
            faults.append(
                f"{field} reaches {sorted(carried)} and not {sorted(set(answers) - carried)}, "
                "and nothing here says why. A field one surface publishes and another drops is "
                "a result carrying its method on some surfaces and not others"
            )
            continue
        if declared.carried_by != carried:
            faults.append(
                f"{field} is declared to reach {sorted(declared.carried_by)} and reaches "
                f"{sorted(carried)}: {declared.reason}"
            )
            continue
        # Whether the surfaces that do carry it agree with each other. A presence check alone
        # passes a field three surfaces publish and two of them contradict, which is the state
        # `registry_version` was found in and the reason this is asked at all.
        agree = carriers_agree_about(answers, field, carried)
        if agree != declared.carriers_agree:
            faults.append(
                f"{field} is recorded as {agreement_reads(declared.carriers_agree)} across "
                f"{sorted(carried)} and measures {agreement_reads(agree)}: "
                f"{{{', '.join(f'{name}={answers[name][field]!r}' for name in sorted(carried))}}}. "
                f"Discharged by {declared.discharged_by}"
            )

    for field, declared in sorted(differ.items()):
        if field in everywhere:
            faults.append(
                f"{field} now reaches every surface, so it is comparable and belongs in "
                f"compared_fields rather than here. Discharged by "
                f"{declared.discharged_by}: {declared.reason}"
            )
        elif field not in somewhere:
            faults.append(
                f"{field} is declared uneven across the surfaces and no surface publishes it: "
                f"{declared.reason}"
            )

    faults += undeclared_reach_faults(everywhere, somewhere, differ, kind)
    return faults


def undeclared_reach_faults(everywhere, somewhere, differ, kind):
    """Every field the document declares is accounted for, and the account is against the
    document rather than against what happened to arrive.

    The registers above measure their universe from the answers, so a field on none of them is
    outside every one of them. This is the direction that cannot be asked that way: the fields
    are read off the struct that declares them, and each has to be compared, asserted another
    way, a declared divergence, or named as reaching no wire at all.
    """
    faults = []
    document = DOCUMENT_OF_KIND[kind]
    declared = keys_a_document_declares(document)
    never = NEVER_ON_THE_WIRE[kind]

    # The control, and it is what makes the parse above evidence rather than a hope. A read that
    # found the wrong struct, or stopped early, or matched nothing, reports a universe the
    # surfaces are not in, and this names the fields that proved it.
    invented = sorted(everywhere - declared)
    if invented:
        faults.append(
            f"every surface publishes {invented} and {document} declares no such field, so the "
            f"{len(declared)} fields this coverage is measured against are not the document's"
        )

    for field in sorted(declared - everywhere - set(differ) - set(never)):
        faults.append(
            f"{document} declares {field} and no answer to this request carries it, and nothing "
            "here says why. A field the whole population leaves empty is dropped from every "
            "wire, so it is compared by nobody and missing from nobody. Add a request that "
            "fills it, or name it in NEVER_ON_THE_WIRE with the request that would"
        )

    for field, absent in sorted(never.items()):
        if field not in declared:
            faults.append(
                f"{field} is named as reaching no wire and {document} declares no such field, "
                f"which reads as coverage and covers nothing: {absent.reason}"
            )
        elif field in somewhere:
            faults.append(
                f"{field} is named as reaching no wire and it is on the wire, so the entry is "
                f"out of date and the field is a divergence or a comparison for real. "
                f"Discharged by {absent.discharged_by}"
            )
    return faults


def carriers_agree_about(answers, field, carried):
    """True, False, or None when one surface carries it and there is nothing to agree about."""
    if len(carried) < 2:
        return None
    return len({canonical(answers[name][field]) for name in carried}) == 1


def article(word):
    """The right article for a kind's name, so a kind added later reads as English."""
    return "an" if word[:1].lower() in "aeiou" else "a"


def agreement_reads(state):
    if state is None:
        return "carried by one surface, with nothing to agree about"
    return "agreeing" if state else "disagreeing"


def write_one(baseline_path, answers, row, source=None):
    """One surface writes the baseline and every surface is then held to it.

    Regenerating from whichever surface answered first would record a defect as the standard
    if that surface had one, so the diff is audited by hand, the discipline
    `crates/plateforce-analysis/tests/resolved-values-baseline.txt` already carries.

    Where the surfaces disagree, the hand audit is not enough and this refuses without
    `--write <surface>`. Whichever surface sorts first is not an answer to which surface is
    right, and taking it would write one surface's defect into the record that is supposed to
    catch it. Both requests added to the population in the commit that wrote this needed the
    argument, one because the browser disagreed and one because Python did, and in both the
    surface named was the one three others agreed with.
    """
    was = compared_fields_in(baseline_path) if pathlib.Path(baseline_path).exists() else []
    fields = compared_fields_measured_from(answers, row.kind)

    # Regeneration widens and never narrows. A surface that stopped publishing a field would
    # otherwise take the field out of the list on the way past, and the gate would go on
    # reporting that every surface agreed, about less.
    dropped = sorted(set(was) - set(fields))
    if dropped:
        raise SystemExit(
            f"plateforce: {dropped} is compared today and some surface no longer publishes it, "
            "so regenerating would narrow this gate. Fix the surface, or take the name out of "
            "compared_fields by hand, which is the one direction that wants a decision"
        )

    faults = coverage_faults(answers, fields, row.kind, row.surfaces)
    if faults:
        for fault in faults:
            print(f"plateforce: {fault}", file=sys.stderr)
        raise SystemExit(
            "plateforce: the surfaces do not carry the shape this gate can compare, so a "
            "baseline written now would record that as the standard"
        )

    projections = {name: projected(answers[name], fields, name) for name in answers}
    agreed = len({canonical(shown) for shown in projections.values()}) == 1
    if source is not None and source not in answers:
        raise SystemExit(
            f"plateforce: {source} is not a surface this run holds, which are {sorted(answers)}"
        )
    if not agreed and source is None:
        reference = sorted(answers)[0]
        for name in sorted(answers):
            moved = differing_paths(projections[reference], projections[name])
            if moved:
                print(
                    f"plateforce: {name} and {reference} differ in {len(moved)} places: "
                    + "; ".join(moved[:6]),
                    file=sys.stderr,
                )
        raise SystemExit(
            f"plateforce: the surfaces do not agree on {baseline_path}, so writing it would "
            "record one of them as the standard by alphabet rather than by a decision. Name "
            "the surface to write from, and say in the commit why that one is right"
        )

    first = source or sorted(answers)[0]
    document = {
        "compared_fields": fields,
        "result": projections[first],
    }
    gained = sorted(set(fields) - set(was))
    chosen = "" if agreed else f", which {len(answers) - 1} others were not all in step with"
    note = (
        f"{baseline_path} written from {first}{chosen}; audit the diff before committing it\n"
        f"compared_fields derived from the surfaces: {len(fields)} fields"
        + (f", {len(gained)} newly covered: {gained}" if gained else ", none newly covered")
    )
    return document, note


def check_one(row, baseline_path, answers):
    """One request against the record it is held to. Faults are returned, never printed.

    Returned so the population can report every request rather than the first one that
    disagreed. A run that stopped at the first fault would say which request is red and leave
    the reader guessing whether the rest were green or merely unasked.
    """
    request_name = row.name
    fields = compared_fields_in(baseline_path)
    with open(baseline_path, encoding="utf-8") as handle:
        committed = json.load(handle)["result"]

    faults = [
        f"{request_name}: {fault}"
        for fault in coverage_faults(answers, fields, row.kind, row.surfaces)
    ]

    for name in sorted(answers):
        reported = projected(answers[name], fields, name)
        if canonical(reported) != canonical(committed):
            moved = differing_paths(committed, reported)
            where = "place" if len(moved) == 1 else "places"
            faults.append(
                f"{request_name}: {name} does not match {baseline_path} in {len(moved)} "
                f"{where}:\n    " + "\n    ".join(moved[:12])
                + ("\n    ..." if len(moved) > 12 else "")
            )

    values = sum(1 for _ in every_number(committed))
    if values == 0:
        faults.append(
            f"{request_name}: the committed record holds no number at all, so a surface "
            "matching it has agreed about nothing"
        )
    return faults, fields, committed, values


def report_one(row, baseline_path, answers, fields, committed, values):
    """What was compared for one request, with the denominator of every count in it."""
    everywhere = fields_every_surface_publishes(answers)
    held = f"{row.equals}'s record" if row.equals else pathlib.Path(baseline_path).name
    listed = len(surfaces_named_in_manifest())
    print(
        f"  {row.name}: {len(answers)} of {listed} listed surfaces computed {held}, "
        f"{values} numbers each, {len(fields)} of {len(everywhere)} fields every surface "
        "asked publishes compared"
    )
    # The surfaces this question cannot be put to, named beside the count above so the count
    # cannot be read as every surface having answered. A surface here answers a narrower
    # question than the record, or none.
    for surface in sorted(surfaces_named_in_manifest() - row.surfaces):
        declared = SURFACES_NOT_ASKED[row.kind][surface]
        print(f"    {surface} is not asked this: {declared.reason}")
    # A field this request leaves empty is compared, and four surfaces holding nothing agree
    # perfectly. So the count is reported beside the one above: a name in the list is coverage
    # of the wire and not yet coverage of a value on this request, and the two read
    # identically without this. Whether the population values it anywhere is the line below.
    hollow = sorted(field for field in fields if is_empty(committed[field]))
    if hollow:
        print(f"    {len(hollow)} of {len(fields)} compared fields empty here: {hollow}")


def write(directory, source=None):
    """Every request that owns a record writes it, and every one is audited by hand.

    A request declared equal to another writes nothing: its claim is that it answers an
    existing record identically, and a second copy of that record would agree with itself
    the moment somebody regenerated one of the two.

    Every document is built before any is written, so a population that refuses part way
    leaves the committed records as they were. A half-regenerated set of baselines is a
    population where some requests hold today's answer and some hold last week's, and nothing
    in the file says which.
    """
    rows = requests_named_in_manifest()
    faults = request_manifest_faults(rows, writing=True)
    if faults:
        for fault in faults:
            print(f"plateforce: {fault}", file=sys.stderr)
        raise SystemExit(
            "plateforce: the request manifest does not describe a population this gate can "
            "compare, so a baseline written now would record that as the standard"
        )

    planned = []
    for row in rows:
        answers = answers_from(directory, row)
        if row.equals:
            print(f"{row.name} is held to {row.equals}'s record and writes none of its own")
            continue
        planned.append(
            (
                str(ROOT / row.baseline),
                *write_one(str(ROOT / row.baseline), answers, row, source),
            )
        )

    for baseline_path, document, note in planned:
        with open(baseline_path, "w", encoding="utf-8") as handle:
            handle.write(json.dumps(document, sort_keys=True, indent=2) + "\n")
        print(note)


def check(directory):
    rows = requests_named_in_manifest()
    faults = request_manifest_faults(rows)
    if faults:
        for fault in faults:
            print(f"plateforce: {fault}", file=sys.stderr)
        raise SystemExit(1)

    reports = []
    committed_by_request = {}
    fields_by_request = {}
    for row in rows:
        answers = answers_from(directory, row)
        baseline_path = str(ROOT / baseline_of(rows, row))
        found, fields, committed, values = check_one(row, baseline_path, answers)
        faults += found
        committed_by_request[row.name] = committed
        fields_by_request[row.name] = fields
        reports.append((row, baseline_path, answers, fields, committed, values))

    faults += hollow_population_faults(committed_by_request, fields_by_request)

    carried = sum(values for *_, values in reports)
    if carried < NUMBERS_THE_POPULATION_CARRIES:
        faults.append(
            f"the {len(rows)} committed records hold {carried} numbers between them, so the "
            "population has agreed about almost nothing"
        )

    if faults:
        for fault in faults:
            print(f"plateforce: {fault}", file=sys.stderr)
        raise SystemExit(1)

    surfaces = len(surfaces_named_in_manifest())
    # Two denominators rather than one. Every request is computed, and every surface a
    # request is asked of answers it, which are different claims once a question exists that
    # some surface's entry point cannot state.
    collected = sum(len(row.surfaces) for row in rows)
    print(
        f"{len(rows)} of {len(rows)} committed requests computed, {collected} of {collected} "
        f"surface answers collected over {surfaces} listed surfaces, {carried} numbers across "
        "the population"
    )
    for report in reports:
        report_one(*report)

    # The denominator of the sentence above, so it cannot be read as a claim about the whole
    # document. Every field is accounted for: compared here, asserted another way, or a
    # divergence the surfaces carry and this comparison cannot reach. Per kind, because a
    # field is in different states on an analysed document and on a swept one.
    for kind in sorted({row.kind for row in rows}):
        asserted = ASSERTED_ANOTHER_WAY[kind]
        print(
            f"  on {article(kind)} {kind} request, {len(asserted)} fields every surface "
            f"publishes are asserted another way rather than compared: {sorted(asserted)}"
        )
        # The document's own denominator, so the counts above cannot be read as the whole of
        # what a result carries. A field named here is one no surface writes at all, which is
        # why no count taken from the answers can see it.
        document = DOCUMENT_OF_KIND[kind]
        declared = keys_a_document_declares(document)
        never = NEVER_ON_THE_WIRE[kind]
        print(
            f"  on {article(kind)} {kind} request, {document} declares {len(declared)} fields "
            f"and {len(declared) - len(never)} of them reach a wire; the {len(never)} that "
            f"reach none are named in NEVER_ON_THE_WIRE: {sorted(never)}"
        )
        for field, absent in sorted(never.items()):
            print(f"    {field} is on no surface's answer: {absent.reason}")
    valued = sorted(
        {
            field
            for name, fields in fields_by_request.items()
            for field in fields
            if not is_empty(committed_by_request[name][field])
        }
    )
    compared = sorted({field for fields in fields_by_request.values() for field in fields})
    print(
        f"  {len(valued)} of {len(compared)} compared fields carry a value on some request; "
        f"the {len(compared) - len(valued)} that carry none anywhere are named in "
        f"EMPTY_ON_EVERY_REQUEST: {sorted(EMPTY_ON_EVERY_REQUEST)}"
    )

    # One request per kind, because the register is per kind and a run that read the first
    # request's answers would print the analysed register's reach against every kind asked.
    for kind in sorted({row.kind for row in rows}):
        answers = next(answers for row, _, answers, *_ in reports if row.kind == kind)
        asked = len(answers)
        everywhere = fields_every_surface_publishes(answers)
        uneven = sorted(set.union(*(set(answer) for answer in answers.values())) - everywhere)
        if not uneven:
            print(f"  on {article(kind)} {kind} request, every field reaches every surface asked")
            continue
        for field in uneven:
            declared = SURFACES_THAT_DIFFER[kind][field]
            print(
                f"  on {article(kind)} {kind} request, {field} reaches "
                f"{sorted(surfaces_publishing(answers, field))} of the {asked} asked, "
                f"{agreement_reads(declared.carriers_agree)}, discharged by "
                f"{declared.discharged_by}"
            )


def every_number(value):
    """A control: a comparison over a document holding no numbers has proven nothing."""
    if isinstance(value, dict):
        for held in value.values():
            yield from every_number(held)
    elif isinstance(value, list):
        for held in value:
            yield from every_number(held)
    elif isinstance(value, (int, float)) and not isinstance(value, bool):
        yield value


def main():
    if len(sys.argv) not in (3, 4):
        raise SystemExit("usage: result_parity.py check|write <answers-directory> [surface]")
    mode, directory = sys.argv[1], sys.argv[2]
    source = sys.argv[3] if len(sys.argv) == 4 else None
    if mode == "write":
        write(directory, source)
    elif mode == "check":
        check(directory)
    else:
        raise SystemExit(f"plateforce: {mode} is not check or write")


if __name__ == "__main__":
    main()
