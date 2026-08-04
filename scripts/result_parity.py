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
"""

import json
import pathlib
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
    """

    name: str
    request: str
    baseline: str
    equals: str


def requests_named_in_manifest():
    """Every request this gate speaks about, read from the file the harness reads.

    The manifest is the only place a request is named. A file in tests/golden that no row
    names is a question nobody asks, which `request_manifest_faults` refuses rather than
    passes over.
    """
    rows = []
    for line in REQUEST_MANIFEST.read_text(encoding="utf-8").splitlines():
        # Not stripped whole, because a row whose third cell is empty ends in a tab and
        # stripping it would read as a row of two cells. A request held to nothing is a
        # refusal with a sentence of its own, and a parse error in its place says the wrong
        # thing about what somebody forgot.
        row = line.rstrip("\n")
        if not row.strip() or row.lstrip().startswith("#"):
            continue
        cells = [cell.strip() for cell in row.split("\t")]
        if len(cells) != 3:
            raise SystemExit(
                f"plateforce: {REQUEST_MANIFEST.name} row {row!r} does not carry a name, a "
                "request file and the record it is held to"
            )
        name, request, held = cells
        if held.startswith("="):
            rows.append(Request(name, request, "", held[1:]))
        else:
            rows.append(Request(name, request, held, ""))
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


def answers_from(directory, request_name):
    """One request's answers, one file per surface, named by the harness that collected them."""
    answers = {}
    for surface in sorted(surfaces_named_in_manifest()):
        path = pathlib.Path(directory) / f"{request_name}.{surface}.json"
        if not path.exists():
            raise SystemExit(
                f"plateforce: {surface} left no answer to {request_name}, so this run holds "
                "fewer surfaces than the manifest names"
            )
        with open(path, encoding="utf-8") as handle:
            answers[surface] = result_in(json.load(handle), f"{surface} on {request_name}")
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


# A field every surface publishes that this gate does not compare against the committed
# document, with the assertion that covers it instead. Naming a field here is not permission
# to stop looking at it: each entry owes a function, `coverage_faults` runs it, and an entry
# without one cannot be written. Widening `compared_fields` is the ordinary answer and this
# is the exception, so an entry states why a committed value would be the wrong instrument.
ASSERTED_ANOTHER_WAY = {
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

class Divergence(NamedTuple):
    """One field some surfaces publish and others do not, as measured rather than as hoped.

    `carriers_agree` is the state the surfaces that DO carry it are in: True when they report
    the same value, False when they do not, and None when only one carries it, because a
    population of one agrees with itself and recording that as agreement would be the shape
    this project keeps catching. It is pinned to measurement, so a divergence being repaired
    reddens this gate exactly as loudly as one appearing. That is the point: the repair is
    supposed to end with the entry moving out of this register.

    `discharged_by` names the work that retires the entry, so an entry cannot quietly become
    permanent by nobody remembering what it was waiting for.
    """

    carried_by: frozenset
    carriers_agree: bool
    discharged_by: str
    reason: str


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
SURFACES_THAT_DIFFER = {
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
    "descriptions": Divergence(
        frozenset({"r"}),
        None,
        "no branch yet, and it wants one",
        "the account each quantity gives of itself. Generated in `descriptions_of`, which "
        "lives in R's binding and in no other surface's, so a terminal, a browser tab and a "
        "notebook receive nothing here",
    ),
    "spread": Divergence(
        frozenset({"cli"}),
        None,
        "nothing outstanding; the other surfaces expose the sweep as a call of its own",
        "how far a number moves across a slot's defensible alternatives. The terminal sweeps "
        "with the analysis; the tab sweeps on its own schedule through a second entry point, "
        "and Python and R expose the sweep as a call of its own",
    ),
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
# `refusals`, `signals` and `warnings` were all here in substance before this population
# existed, empty on the one request the gate asked. They are valued now: the interrupted
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


def compared_fields_measured_from(answers):
    """What `compared_fields` should name: everything the surfaces all publish, less the
    fields asserted another way.

    Derived rather than maintained, which is the whole point. Read out of the baseline it
    checks, this list could only ever be widened by hand, so a field added to all four
    surfaces stayed invisible to the gate until somebody noticed.
    """
    return sorted(fields_every_surface_publishes(answers) - set(ASSERTED_ANOTHER_WAY))


def coverage_faults(answers, fields):
    """Every field a surface publishes is compared here, asserted another way, or a declared
    divergence. A field in none of the three is a field nothing looks at.

    This gate prints that four surfaces computed one result. A reader takes that to be about
    the result, not about the six fields of it somebody listed, so the gate refuses rather
    than publish a verdict narrower than its own sentence.
    """
    faults = []
    named = surfaces_named_in_manifest()
    if set(answers) != named:
        faults.append(
            f"the manifest names {sorted(named)} and this run holds {sorted(answers)}, so "
            "nothing below speaks for the surfaces this gate claims"
        )
        return faults

    everywhere = fields_every_surface_publishes(answers)
    somewhere = set.union(*(set(answer) for answer in answers.values()))

    unread = sorted(everywhere - set(fields) - set(ASSERTED_ANOTHER_WAY))
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

    for field in sorted(set(fields) & set(ASSERTED_ANOTHER_WAY)):
        faults.append(
            f"{field} is compared against the committed document and also declared asserted "
            "another way, so one of the two is wrong"
        )

    for field, (reason, assertion) in sorted(ASSERTED_ANOTHER_WAY.items()):
        if field not in somewhere:
            faults.append(
                f"{field} is declared asserted another way and no surface publishes it, which "
                f"reads as coverage and covers nothing: {reason}"
            )
            continue
        faults += assertion(answers)

    for field in sorted(somewhere - everywhere):
        carried = surfaces_publishing(answers, field)
        declared = SURFACES_THAT_DIFFER.get(field)
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

    for field, declared in sorted(SURFACES_THAT_DIFFER.items()):
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

    return faults


def carriers_agree_about(answers, field, carried):
    """True, False, or None when one surface carries it and there is nothing to agree about."""
    if len(carried) < 2:
        return None
    return len({canonical(answers[name][field]) for name in carried}) == 1


def agreement_reads(state):
    if state is None:
        return "carried by one surface, with nothing to agree about"
    return "agreeing" if state else "disagreeing"


def write_one(baseline_path, answers, source=None):
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
    fields = compared_fields_measured_from(answers)

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

    faults = coverage_faults(answers, fields)
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


def check_one(request_name, baseline_path, answers):
    """One request against the record it is held to. Faults are returned, never printed.

    Returned so the population can report every request rather than the first one that
    disagreed. A run that stopped at the first fault would say which request is red and leave
    the reader guessing whether the rest were green or merely unasked.
    """
    fields = compared_fields_in(baseline_path)
    with open(baseline_path, encoding="utf-8") as handle:
        committed = json.load(handle)["result"]

    faults = [f"{request_name}: {fault}" for fault in coverage_faults(answers, fields)]

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
    print(
        f"  {row.name}: {len(answers)} of {len(answers)} surfaces computed {held}, "
        f"{values} numbers each, {len(fields)} of {len(everywhere)} fields every surface "
        "publishes compared"
    )
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
        answers = answers_from(directory, row.name)
        if row.equals:
            print(f"{row.name} is held to {row.equals}'s record and writes none of its own")
            continue
        planned.append((str(ROOT / row.baseline), *write_one(str(ROOT / row.baseline), answers, source)))

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
        answers = answers_from(directory, row.name)
        baseline_path = str(ROOT / baseline_of(rows, row))
        found, fields, committed, values = check_one(row.name, baseline_path, answers)
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
    print(
        f"{surfaces} of {surfaces} surfaces computed {len(rows)} of {len(rows)} committed "
        f"requests, {carried} numbers across the population"
    )
    for report in reports:
        report_one(*report)

    # The denominator of the sentence above, so it cannot be read as a claim about the whole
    # document. Every field is accounted for: compared here, asserted another way, or a
    # divergence the surfaces carry and this comparison cannot reach.
    print(
        f"  {len(ASSERTED_ANOTHER_WAY)} fields every surface publishes are asserted another "
        f"way rather than compared: {sorted(ASSERTED_ANOTHER_WAY)}"
    )
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

    answers = reports[0][2]
    everywhere = fields_every_surface_publishes(answers)
    for field in sorted(set.union(*(set(answer) for answer in answers.values())) - everywhere):
        declared = SURFACES_THAT_DIFFER[field]
        print(
            f"  {field} reaches {sorted(surfaces_publishing(answers, field))} of the "
            f"{surfaces}, {agreement_reads(declared.carriers_agree)}, discharged by "
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
