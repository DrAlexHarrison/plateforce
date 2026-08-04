#!/usr/bin/env python3
"""Every choice between named alternatives is named in the registry, in the registry's words.

A rule that picks between named alternatives moves the number, and a reader who cannot see
which alternatives exist cannot tell a stated choice from a silent one. Three ways that breaks,
each measured here against the running software rather than against a list written down beside
it:

    A  a parameter the registry files as an enumeration and never says what it takes
    B  a name a rule records that its registry row does not declare, so a reader holding a
       result cannot look the name up at all
    C  a value the software accepts that the registry's row does not list

The population is the entries a result can name, which is larger than the table of bindings:
a single run records the composition operators and the conditioning rule alongside the three
landmark rules, and each of those is a registry row with parameters of its own.

The accepted values are read out of the software's own refusal. Handed a value no rule takes,
`Resolution::enumerated` refuses with the list it does take, so the list this checks against
is the one the code branches on rather than a transcription of it.

Run: python3 scripts/check-enumerated-values-are-data.py
Exit: 0 clean, 1 violations, 3 nothing was measured.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = ROOT / "registry"
TRIAL = ROOT / "crates/plateforce-conformance/fixtures/subject01_trial1.force.txt"
SAMPLE_RATE_HZ = "1200"

# A value no rule takes, so every enumerated name refuses and names what it does take.
UNTAKEN = "__not_a_value_any_rule_takes__"

# The three landmark slots, each named by the construct a preset binds it under.
SPINE = ("system_weight", "movement_onset", "takeoff")

# A rule under each landmark construct that runs on the fixture with its own defaults, held
# fixed while another slot is swept. Any working rule serves; these are the three the
# conformance fixtures already exercise.
BASELINE = {
    "system_weight": "bwepoch.fixed_window",
    "movement_onset": "onset.threshold.noise_relative",
    "takeoff": "takeoff.threshold.absolute_force",
}


def run(arguments: list[str]) -> tuple[int, str]:
    finished = subprocess.run(
        arguments, capture_output=True, text=True, cwd=ROOT, env={"NO_COLOR": "1", "PATH": "/usr/bin:/bin"}
    )
    return finished.returncode, finished.stdout + finished.stderr


def binary() -> Path:
    """The binary rebuilt from the tree, every run.

    Reusing whatever was already at this path reads the software as it was when somebody last
    built it, and a check that reports on a stale artefact reports on a tree nobody has.
    """
    subprocess.run(["cargo", "build", "-q", "-p", "plateforce-cli"], cwd=ROOT, check=True)
    built = ROOT / "target/debug/plateforce"
    if not built.is_file():
        raise SystemExit(f"plateforce: no runnable binary at {built}, so this check did not run")
    return built


def registry_methods() -> dict[str, dict]:
    methods: dict[str, dict] = {}
    for path in sorted((REGISTRY / "methods").glob("*.toml")):
        for method in tomllib.loads(path.read_text()).get("method", []):
            methods[method["id"]] = method
    return methods


def numeric_bindings(method: dict | None) -> dict[str, float]:
    """Every numeric value a rule needs, taken from its own row.

    A rule that declines for want of a number records nothing, and a run that recorded
    nothing looks exactly like a rule with no choices in it. The published value stands in
    where a row states one and declares no default, which is the case the registry files as
    a parameter the reader must answer.
    """
    if method is None:
        return {}
    supplied: dict[str, float] = {}
    for parameter in method.get("parameter", []):
        if parameter.get("unit") == "enumeration":
            continue
        if parameter.get("default") is not None:
            supplied[parameter["name"]] = float(parameter["default"])
        elif parameter.get("published_values"):
            supplied[parameter["name"]] = float(parameter["published_values"][0])
    return supplied


def preset_document(bindings: list[dict]) -> str:
    """One pipeline, written the way the registry writes one."""
    lines = [
        "[[preset]]",
        'id = "probe"',
        'title = "Probe"',
        'description = "Probe"',
        "",
    ]
    for binding in bindings:
        lines.append("[[preset.binding]]")
        lines.append(f'construct = "{binding["construct"]}"')
        lines.append(f'method_id = "{binding["method_id"]}"')
        if binding.get("composed_from"):
            lines.append(f'composed_from = "{binding["composed_from"]}"')
        if binding.get("parameters"):
            written = ", ".join(f"{name} = {value}" for name, value in sorted(binding["parameters"].items()))
            lines.append(f"parameters = {{ {written} }}")
        if binding.get("options"):
            written = ", ".join(f'{name} = "{value}"' for name, value in sorted(binding["options"].items()))
            lines.append(f"options = {{ {written} }}")
        lines.append("")
    lines.extend(
        [
            "[[preset.citation]]",
            'key = "probe"',
            'role = "proposes"',
            'reference = "Probe"',
            "obtained = true",
            "",
        ]
    )
    return "\n".join(lines)


class Engine:
    """The built binary, run against a copy of the registry a probe preset can be written into."""

    def __init__(self, workspace: Path) -> None:
        self.binary = binary()
        self.registry = workspace / "registry"
        shutil.copytree(REGISTRY, self.registry)
        self.methods = registry_methods()

    def analyse(self, bindings: list[dict], derived: list[str]) -> dict | None:
        (self.registry / "presets" / "probe.toml").write_text(preset_document(bindings))
        arguments = [
            str(self.binary),
            "--registry",
            str(self.registry),
            "--format",
            "json",
            "analyse",
            str(TRIAL),
            "--column",
            "0",
            "--sample-rate-hz",
            SAMPLE_RATE_HZ,
            "--sentinel",
            "none",
            "--preset",
            "probe",
        ]
        for assignment in derived:
            arguments.extend(["--derive", assignment])
        _, output = run(arguments)
        first = output.strip().splitlines()
        if not first:
            return None
        try:
            document = json.loads(first[0])
        except json.JSONDecodeError:
            return None
        return document.get("ok") or document.get("refusal")

    def bindings_for(self, construct: str, method_id: str, options: dict[str, str] | None = None) -> dict:
        method = self.methods.get(method_id)
        binding = {
            "construct": construct,
            "method_id": method_id,
            "parameters": numeric_bindings(method),
        }
        if method is None:
            # A composition carries no row, so the entry it composes supplies the numbers and
            # names the row a preset is checked against.
            binding["composed_from"] = self.composition_base(method_id)
            binding["parameters"] = numeric_bindings(self.methods.get(binding["composed_from"]))
        if options:
            binding["options"] = options
        return binding

    def composition_base(self, method_id: str) -> str:
        family = method_id.rsplit(".", 1)[0]
        for candidate in self.methods:
            if candidate.startswith(family) and candidate != method_id:
                return candidate
        return method_id


def spine_bindings(engine: Engine, construct: str, method_id: str, options: dict[str, str] | None) -> list[dict]:
    bindings = []
    for slot in SPINE:
        if slot == construct:
            bindings.append(engine.bindings_for(slot, method_id, options))
        else:
            bindings.append(engine.bindings_for(slot, BASELINE[slot]))
    return bindings


def recorded_choices(result: dict, entry_ids: set[str]) -> dict[str, dict[str, str]]:
    """What each rule in one result recorded that is a choice between named alternatives.

    Two kinds of recorded word are not that, and both resolve for a reader already. A value
    that parses as a number is a quantity, whatever the row calls it. A value that is itself
    a registry id is one rule naming another, which is how the four integration dimensions
    travel: a jump-height rule states none of them and records the entry chosen for each.
    """
    per_method: dict[str, dict[str, str]] = {}
    for bound in result.get("bound_methods", []):
        chosen = {}
        for name, value in bound.get("bound_parameters", []):
            if value in entry_ids:
                continue
            try:
                float(value)
            except ValueError:
                chosen[name] = value
        per_method.setdefault(bound["method_id"], {}).update(chosen)
    return per_method


def offered_for(result: dict, name: str) -> list[str] | None:
    """The values the software says it takes, read off the refusal it raised."""
    for refusal in result.get("refusals", []) or []:
        if refusal.get("parameter") == name and refusal.get("code") == "value_not_accepted":
            return refusal.get("available") or []
    if result.get("parameter") == name and result.get("code") == "value_not_accepted":
        return result.get("available") or []
    return None


def main() -> int:  # noqa: C901
    if not TRIAL.is_file():
        print(f"plateforce: no trial at {TRIAL}, so this check did not run", file=sys.stderr)
        return 3

    with tempfile.TemporaryDirectory() as directory:
        engine = Engine(Path(directory))
        return check(engine)


def check(engine: Engine) -> int:  # noqa: C901
    capability_code, capability_output = run(
        [str(engine.binary), "--registry", str(engine.registry), "--format", "json", "capability"]
    )
    if capability_code != 0:
        print("plateforce: capability did not answer, so this check measured nothing", file=sys.stderr)
        return 3
    rules = json.loads(capability_output)["ok"]["methods"]
    entry_ids = set(engine.methods)

    # A composition is an entry with an operator bound onto it and carries no row of its own,
    # so its parameters are the composed entry's and that is the row a reader is sent to.
    row_of = {rule["id"]: rule.get("composed_from") or rule["id"] for rule in rules}

    # Every rule this build runs, swept one slot at a time so each one reaches the record.
    runs: list[tuple[list[dict], list[str]]] = []
    for rule in rules:
        construct = rule["construct"]
        if construct in SPINE:
            runs.append((spine_bindings(engine, construct, rule["id"], None), []))
        else:
            bindings = [engine.bindings_for(slot, BASELINE[slot]) for slot in SPINE]
            bindings.append(engine.bindings_for(construct, rule["id"]))
            runs.append((bindings, [f"{construct}={rule['id']}"]))

    # Which construct each entry's choices are stated under, so a probe reaches the rule that
    # reads them rather than the slot next to it.
    population: dict[str, dict[str, str]] = {}
    construct_of: dict[str, str] = {}
    for bindings, derived in runs:
        result = engine.analyse(bindings, derived)
        if result is None:
            continue
        swept = bindings[-1]["construct"] if derived else None
        for method_id, chosen in recorded_choices(result, entry_ids).items():
            population.setdefault(method_id, {}).update(chosen)
            if method_id not in construct_of:
                construct_of[method_id] = swept or slot_of(method_id, bindings, result)

    # The control, and it is per rule rather than a total: a sweep that reached forty entries
    # while missing the one rule whose values are unnamed reports the same total as one that
    # reached them all. A composition is recorded under the row it composes, so that is the id
    # its run is looked for under.
    unreached = sorted(rule["id"] for rule in rules if row_of[rule["id"]] not in population)
    if unreached:
        print(
            f"plateforce: {len(unreached)} of {len(rules)} rules this build runs never reached the "
            f"record, so this check did not measure them: {', '.join(unreached)}",
            file=sys.stderr,
        )
        return 3

    # The second control. A registry whose enumerations are all empty and one whose values are
    # all present read the same to a checker that never found an enumeration to look at.
    declared = {
        (method_id, parameter["name"]): parameter
        for method_id in population
        for parameter in engine.methods.get(row_of.get(method_id, method_id), {}).get("parameter", [])
        if parameter.get("unit") == "enumeration"
    }
    if not declared:
        print(
            "plateforce: no entry a result can name declares an enumeration, so there was "
            "nothing here to check",
            file=sys.stderr,
        )
        return 3

    unnamed = sorted(key for key, parameter in declared.items() if not parameter.get("value"))
    unlabelled = sorted(
        (method_id, name, value["key"])
        for (method_id, name), parameter in declared.items()
        for value in parameter.get("value", [])
        if not (value.get("label") or "").strip()
    )

    # A name recorded as a word is a choice between alternatives, and a reader who finds it in
    # a result looks it up on the entry that recorded it.
    undeclared = sorted(
        (method_id, row_of.get(method_id, method_id), name)
        for method_id, chosen in population.items()
        for name in chosen
        if (method_id, name) not in declared
    )

    # Every value the software takes, asked of the software one name at a time.
    unlisted: list[tuple[str, str, str]] = []
    probed = 0
    unprobeable: list[tuple[str, str]] = []
    for (method_id, name), parameter in sorted(declared.items()):
        construct = construct_of.get(method_id)
        if construct is None:
            continue
        if construct in SPINE:
            bindings = spine_bindings(engine, construct, bound_in(construct, method_id), {name: UNTAKEN})
            derived: list[str] = []
        else:
            bindings = [engine.bindings_for(slot, BASELINE[slot]) for slot in SPINE]
            bindings.append(engine.bindings_for(construct, method_id, {name: UNTAKEN}))
            derived = [f"{construct}={method_id}"]
        result = engine.analyse(bindings, derived)
        offered = offered_for(result, name) if result else None
        if offered is None:
            # A name the record carries and no rule reads as a choice. Its values are the ones
            # observed in the sweep.
            observed = {population[method_id][name]} if name in population.get(method_id, {}) else set()
            unprobeable.append((method_id, name))
            offered = sorted(observed)
        else:
            probed += 1
        listed = {value["key"] for value in parameter.get("value", [])}
        unlisted.extend((method_id, name, value) for value in sorted(set(offered) - listed))

    print(f"{'population':<30}{len(population)} entries a result can name, of {len(rules)} rules this build runs")
    print(f"{'':<30}query: analyse over every rule, collecting bound_methods[].method_id")
    print(f"{'enumerations declared':<30}{len(declared)} on {len({key[0] for key in declared})} of those entries")
    print(f"{'values named':<30}{sum(len(p.get('value', [])) for p in declared.values())}")
    print(f"{'accepted lists read back':<30}{probed} of {len(declared)}, from the refusal each name raises")
    print(f"{'choices recorded':<30}{sum(len(chosen) for chosen in population.values())}")

    violations = 0
    for method_id, name in unnamed:
        print(f"plateforce: {method_id} files '{name}' as an enumeration and names no value it takes", file=sys.stderr)
        violations += 1
    for method_id, name, key in unlabelled:
        print(f"plateforce: {method_id} parameter '{name}' offers '{key}' with no label a reader can read", file=sys.stderr)
        violations += 1
    for method_id, row, name in undeclared:
        where = f"{row}, the row it composes," if row != method_id else "its registry row"
        print(
            f"plateforce: {method_id} records '{name}' and {where} declares no such parameter, "
            "so a reader holding that result cannot look the name up",
            file=sys.stderr,
        )
        violations += 1
    for method_id, name, value in unlisted:
        print(
            f"plateforce: {method_id} accepts '{name}' = '{value}' and its registry row does not "
            "list that value",
            file=sys.stderr,
        )
        violations += 1

    if violations:
        print(f"plateforce: {violations} enumerated choices are not registry data", file=sys.stderr)
        return 1

    for method_id, name in unprobeable:
        print(f"{'':<30}{method_id} '{name}' is recorded rather than chosen, checked against what it emitted")
    print("every enumerated choice on every entry a result can name is registry data")
    return 0


def bound_in(construct: str, method_id: str) -> str:
    """The id to bind into a landmark slot so the entry that records the name runs.

    A composition operator has no binding of its own: it runs underneath the threshold rule
    that composes it, so the slot takes the baseline rule and the operator records beside it.
    """
    return method_id if method_id.count(".") >= 1 and ".op." not in method_id else BASELINE[construct]


def slot_of(method_id: str, bindings: list[dict], result: dict) -> str:
    """Which landmark slot an entry recorded under, read off the run that recorded it."""
    for binding in bindings:
        if binding["method_id"] == method_id or binding.get("composed_from") == method_id:
            return binding["construct"]
    family = method_id.split(".", 1)[0]
    if family in ("onset",):
        return "movement_onset"
    if family in ("takeoff",):
        return "takeoff"
    return "system_weight"


if __name__ == "__main__":
    sys.exit(main())
