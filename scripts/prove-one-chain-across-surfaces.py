"""One response, every consumer that publishes the chain behind a number, one tree.

`plateforce_analysis::chain_of` is the one derivation. Four sites built the tree for
themselves before it existed and the four disagreed, so one number arrived at a folder run and
at a notebook resting on different rules. `every_consumer_reads_one_chain.rs` holds the call
sites: each consumer names the derivation and none assembles a chain of its own. What a source
scan cannot see is what a consumer does with the tree after it has it, because that is a value
rather than a line of source.

This asks the surfaces. Two of them hand a caller a tree, and neither can be reached from
`cargo test`: R links the engine through the copies `sync-engine.sh` makes, and the notebook
needs an interpreter. So the comparison runs here, over the artefacts the two surface scripts
build, and refuses rather than skips when either is absent: a skip reads exactly like a pass.

The terminal names the population. Asking each arm which quantities it can answer for and
comparing what both happened to reach would report agreement over the intersection, and a
quantity that fell off one surface would leave no trace. So the terminal is asked what this
request reports, and an arm that cannot answer for one of those names fails this run.

Differences are reported against a register rather than against zero, and the register is
pinned to measurement in both directions. A difference nobody recorded reddens this, and so
does the repair of a recorded one, because a repair that nobody notices leaves a register
entry describing a state that has passed.

    python3 scripts/prove-one-chain-across-surfaces.py
"""

import json
import os
import pathlib
import random
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
R_LIBRARY = ROOT / "target" / "r-surface" / "library"
PYTHON_SURFACE = ROOT / "target" / "python-surface" / "venv" / "bin" / "python"
TERMINAL = ROOT / "target" / "debug" / "plateforce"

SAMPLE_RATE_HZ = 1200
SYSTEM_WEIGHT_NEWTONS = 60.0 * 9.80665

# The rules and the values they read, written once here and typed by each arm in its own
# spelling, which is what `scripts/result-from-*.sh` do with the parity request.
WEIGHING = ["--weighing", "bwepoch.fixed_window", "--set", "weighing.duration=1.0"]
ONSET = ["--onset", "onset.threshold.noise_relative", "--set", "onset.k=5.0"]
TAKEOFF = ["--takeoff", "takeoff.threshold.absolute_force", "--set", "takeoff.threshold_n=20.0"]


def absent(arm, builds_it):
    """An arm that is not built, named with what builds it.

    A comparison that quietly drops an arm reports agreement between the arms it kept.
    """
    return SystemExit(
        f"plateforce: the {arm} arm is not built from this tree. Run {builds_it}"
    )


def a_jump_that_lands(into):
    """A countermovement jump that leaves the plate and lands back on it.

    Written here rather than read from the corpus, because no trial from it appears in this
    repository. Every landmark is placed and the quantities resting on a landing report a
    number, so the trees this compares are as deep as this build makes them.
    """
    generator = random.Random(11)
    samples = [SYSTEM_WEIGHT_NEWTONS + generator.gauss(0.0, 1.0) for _ in range(SAMPLE_RATE_HZ)]
    unloading = int(0.25 * SAMPLE_RATE_HZ)
    samples += [
        SYSTEM_WEIGHT_NEWTONS + (380.0 - SYSTEM_WEIGHT_NEWTONS) * index / (unloading - 1)
        for index in range(unloading)
    ]
    push = int(0.25 * SAMPLE_RATE_HZ)
    samples += [380.0 + (1500.0 - 380.0) * index / (push - 1) for index in range(push)]
    samples += [0.0] * int(0.5 * SAMPLE_RATE_HZ)
    samples += [2400.0] * int(0.2 * SAMPLE_RATE_HZ)
    samples += [SYSTEM_WEIGHT_NEWTONS] * int(0.5 * SAMPLE_RATE_HZ)
    into.write_text("\n".join(f"{sample:.6f}" for sample in samples) + "\n")
    return len(samples)


def quantities_this_request_reports(trace):
    """What the terminal says this request produced, which is the population every arm answers.

    Read off the run rather than from a list here: a quantity this build stops reporting
    leaves this population by being absent from the answer, and one it starts reporting joins
    it without an edit.
    """
    if not TERMINAL.is_file():
        raise absent("terminal", "cargo build -p plateforce-cli")
    finished = subprocess.run(
        [
            str(TERMINAL),
            "--format",
            "json",
            "analyse",
            str(trace),
            "--column",
            "0",
            "--sample-rate-hz",
            str(SAMPLE_RATE_HZ),
            "--sentinel",
            "none",
            "--delimiter",
            "\t",
            *WEIGHING,
            *ONSET,
            *TAKEOFF,
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if finished.returncode != 0:
        raise SystemExit(
            f"plateforce: the terminal exited {finished.returncode}\n{finished.stdout}"
            f"{finished.stderr}"
        )
    answered = json.loads(finished.stdout)["ok"]
    return sorted(
        metric["key"] for metric in answered["metrics"] if metric.get("value") is not None
    )


def tree_from_r(trace):
    if not (R_LIBRARY / "plateforce" / "DESCRIPTION").is_file():
        raise absent("R", "bash scripts/r-surface.sh --install-only")
    finished = subprocess.run(
        ["Rscript", "--vanilla", str(ROOT / "scripts" / "chain-from-r.R"), str(trace)],
        cwd=ROOT,
        env={**os.environ, "R_LIBS": str(R_LIBRARY)},
        capture_output=True,
        text=True,
    )
    if finished.returncode != 0:
        raise SystemExit(f"plateforce: the R arm exited {finished.returncode}\n{finished.stderr}")
    return json.loads(finished.stdout)


def tree_from_python(trace, quantities):
    if not PYTHON_SURFACE.is_file():
        raise absent("Python", "bash scripts/install-python-wheel.sh")
    finished = subprocess.run(
        [
            str(PYTHON_SURFACE),
            str(ROOT / "scripts" / "chain-from-python.py"),
            str(trace),
            *quantities,
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if finished.returncode != 0:
        raise SystemExit(
            f"plateforce: the Python arm exited {finished.returncode}\n{finished.stderr}"
        )
    return json.loads(finished.stdout)


def steps_of(tree, quantity, depth=0, into=None):
    """Every step of one chain as (quantity, depth, method_id), in the order it is walked."""
    into = [] if into is None else into
    into.append((quantity, depth, tree["method_id"]))
    for below in tree["depends_on"]:
        steps_of(below, quantity, depth + 1, into)
    return into


def parameters_of(tree, quantity, depth=0, into=None):
    """Every parameter record as (quantity, depth, method_id, name), values excluded."""
    into = [] if into is None else into
    for name in tree["parameters"]:
        into.append((quantity, depth, tree["method_id"], name))
    for below in tree["depends_on"]:
        parameters_of(below, quantity, depth + 1, into)
    return into


# What the surfaces are measured to disagree about, each entry naming the work that ends it.
#
# `Analysis::one` in `crates/plateforce-python/src/analysis.rs` appends the gravity the
# analysis ran under to the root step of the quantities that rest on it, after reading the tree
# from the one derivation. A rule may record only a parameter its own registry entry declares,
# and of the twelve rules reading that value one declares it, so on every other surface the
# number that moved these reaches no step of the chain at all. The information is real. Its
# home is `chain_of`, which would put it on every surface at once and empty this register.
KNOWN_DIFFERENCES = {
    ("python", "jump_height_from_flight_time_meters", 0, "jumpheight.takeoff.flight_time",
     "gravity_meters_per_second_squared"),
    ("python", "jump_height_from_takeoff_meters", 0, "jumpheight.takeoff.impulse_momentum",
     "gravity_meters_per_second_squared"),
    ("python", "reactive_strength_index_modified", 0, "rsimod.jh_tov_over_ttt",
     "gravity_meters_per_second_squared"),
    ("python", "system_mass_kilograms", 0, "bwepoch.fixed_window",
     "gravity_meters_per_second_squared"),
    ("python", "takeoff_velocity_meters_per_second", 0,
     "impulse.net_vertical.as_performance_determinant",
     "gravity_meters_per_second_squared"),
}

# The floor the populations are held to, so a run over a shrunken answer cannot read as
# agreement. Both are this build's own figures, taken with the request above.
QUANTITIES_AT_LEAST = 8
STEPS_AT_LEAST = 20
DEEPEST_AT_LEAST = 2


def main():
    with tempfile.TemporaryDirectory(prefix="plateforce-one-chain-") as folder:
        trace = pathlib.Path(folder) / "a-jump-that-lands.force.txt"
        samples = a_jump_that_lands(trace)
        population = quantities_this_request_reports(trace)
        print(f"trace of {samples} samples, {len(population)} quantities reported")
        arms = {
            "r": tree_from_r(trace),
            "python": tree_from_python(trace, population),
        }

    # The control on everything below. Two arms that answered nothing agree about nothing.
    if len(population) < QUANTITIES_AT_LEAST:
        raise SystemExit(
            f"plateforce: this request reported {len(population)} quantities, under the "
            f"{QUANTITIES_AT_LEAST} a comparison needs"
        )
    for arm, trees in sorted(arms.items()):
        missing = [name for name in population if name not in trees]
        if missing:
            raise SystemExit(
                f"plateforce: the terminal reports {missing} and {arm} publishes no chain "
                "for them"
            )
        extra = sorted(set(trees) - set(population))
        if extra:
            raise SystemExit(
                f"plateforce: {arm} publishes a chain for {extra}, which this request does "
                "not report"
            )

    # The shape first: which rule sits where, under which number.
    shapes = {
        arm: {name: steps_of(trees[name], name) for name in population}
        for arm, trees in arms.items()
    }
    steps = sum(len(rows) for rows in shapes["r"].values())
    deepest = max(row[1] for rows in shapes["r"].values() for row in rows)
    print(f"steps compared: {steps} over {len(population)} quantities, deepest {deepest}")
    if steps < STEPS_AT_LEAST or deepest < DEEPEST_AT_LEAST:
        raise SystemExit(
            f"plateforce: {steps} steps and a deepest chain of {deepest}, so this compares "
            "flat lists rather than trees"
        )
    disagreeing = [
        f"{name}:\n      r      {shapes['r'][name]}\n      python {shapes['python'][name]}"
        for name in population
        if shapes["r"][name] != shapes["python"][name]
    ]
    if disagreeing:
        raise SystemExit("plateforce: one response, two trees:\n    " + "\n    ".join(disagreeing))

    # Then what each step says produced its number.
    named = {
        arm: {row for name in population for row in parameters_of(trees[name], name)}
        for arm, trees in arms.items()
    }
    on_every_arm = set.intersection(*named.values())
    differences = {
        (arm, *row)
        for arm, rows in named.items()
        for row in rows - on_every_arm
    }
    print(
        f"parameter records: {len(set.union(*named.values()))} across both arms, "
        f"{len(on_every_arm)} on both"
    )
    if not on_every_arm:
        raise SystemExit(
            "plateforce: no parameter record appears on both arms, so the comparison below "
            "is between two empty sets"
        )

    appeared = differences - KNOWN_DIFFERENCES
    repaired = KNOWN_DIFFERENCES - differences
    if appeared:
        raise SystemExit(
            "plateforce: a surface names a choice behind a number that the other does not, "
            "and nothing records it:\n    "
            + "\n    ".join(str(row) for row in sorted(appeared))
        )
    if repaired:
        raise SystemExit(
            "plateforce: these are recorded as differences and the surfaces now agree, so "
            "the register describes a state that has passed:\n    "
            + "\n    ".join(str(row) for row in sorted(repaired))
        )
    print(f"differences recorded and still measured: {len(differences)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
