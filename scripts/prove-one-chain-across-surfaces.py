"""One response, every consumer that publishes the chain behind a number, one tree.

`plateforce_analysis::chain_of` is the one derivation. Four sites built the tree for
themselves before it existed and the four disagreed, so one number arrived at a folder run and
at a notebook resting on different rules. `every_consumer_reads_one_chain.rs` holds the call
sites: each consumer names the derivation and none assembles a chain of its own. What a source
scan cannot see is what a consumer does with the tree after it has it, because that is a value
rather than a line of source.

This asks the surfaces, at the reader's hand rather than at the boundary each surface calls.
Two consumers that both reach one Rust function through one call prove the function is
deterministic and nothing about themselves: the interesting failure is a consumer that reads
the shared tree and then adds to it, or drops from it, on the way out. Both have happened. The
notebook added the analysis gravity to five root steps; the folder run took each row's
parameters from `bound_methods` rather than from the step it was standing on, and dropped the
same value.

Three arms, covering the four converted consumers. R goes through both of them, the boundary
crate and the package. The notebook is the third. The folder run is the fourth and publishes a
relation rather than a tree, so it is compared on what a relation can express: which rule sits
at which depth under which number, and what each of those says produced it.

None of the three can be reached from `cargo test`. R links the engine through the copies
`sync-engine.sh` makes, the notebook needs an interpreter, and the folder run writes files. So
the comparison runs here, over the artefacts the surface scripts build, and refuses rather than
skips when one is absent: a skip reads exactly like a pass.

The terminal names the population. Asking each arm which quantities it can answer for and
comparing what they happened to reach would report agreement over the intersection, and a
quantity that fell off one surface would leave no trace. So the terminal is asked what this
request reports, and an arm that cannot answer for one of those names fails this run.

Differences are reported against a register rather than against zero, and the register is
pinned to measurement in both directions. A difference nobody recorded reddens this, and so
does the repair of a recorded one, because a repair that nobody notices leaves a register
entry describing a state that has passed.

    python3 scripts/prove-one-chain-across-surfaces.py
"""

import csv
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


def tree_from_the_folder_run(trace, folder):
    """What the folder run's own reader receives: `provenance.csv`, as the two sets below.

    A relation, not a tree, so it carries the depth of each step and not the parent that put it
    there. Compared on what it can express rather than reshaped into a tree it does not
    publish, because inventing the missing parent link here would compare this arm against a
    guess rather than against its reader.
    """
    if not TERMINAL.is_file():
        raise absent("folder run", "cargo build -p plateforce-cli")
    trials = folder / "trials"
    trials.mkdir()
    trace.rename(trials / trace.name)
    out = folder / "written"
    finished = subprocess.run(
        [
            str(TERMINAL),
            "batch",
            str(trials),
            "--out-dir",
            str(out),
            "--trial-suffix",
            ".force.txt",
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
            f"plateforce: the folder run exited {finished.returncode}\n{finished.stdout}"
            f"{finished.stderr}"
        )
    steps, named = set(), set()
    with (out / "provenance.csv").open() as written:
        for row in csv.DictReader(written):
            where = (row["quantity"], int(row["depth"]), row["method_id"])
            steps.add(where)
            if row["parameter"]:
                named.add((*where, row["parameter"]))
    return steps, named


def steps_of(tree, quantity, depth=0, into=None):
    """Every step of one chain as (quantity, depth, method_id), in the order it is walked."""
    into = [] if into is None else into
    into.append((quantity, depth, tree["method_id"]))
    for below in tree["depends_on"]:
        steps_of(below, quantity, depth + 1, into)
    return into


def parameters_of(tree, quantity, depth=0, into=None, fields=("parameters",)):
    """Every record as (quantity, depth, method_id, name), values excluded.

    `fields` selects numbers, named alternatives, or both. Both is what the folder run's one
    column can be compared against; apart is what tells a surface publishing a number as a
    named alternative from one that does not.
    """
    into = [] if into is None else into
    for field in fields:
        for name in tree[field]:
            into.append((quantity, depth, tree["method_id"], name))
    for below in tree["depends_on"]:
        parameters_of(below, quantity, depth + 1, into, fields)
    return into


# What the surfaces are measured to disagree about, each entry naming the work that ends it.
#
# Empty, and it has been full. Five entries recorded the analysis gravity reaching the root step
# of five quantities on the notebook and on no other surface, because `Analysis::one` added it
# after reading the tree. The addition was right and its home was the derivation: it is
# `chain_of`'s now, so the same record reaches every consumer and these five closed together.
# The register reddens on a difference nobody recorded and on the repair of a recorded one, so
# emptying it was part of the repair rather than a step after it.
KNOWN_DIFFERENCES = set()

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
        trees = {
            "r": tree_from_r(trace),
            "python": tree_from_python(trace, population),
        }
        # Last, because it moves the trace into the folder it reads.
        folder_run_steps, folder_run_named = tree_from_the_folder_run(
            trace, pathlib.Path(folder)
        )

    # The control on everything below. Arms that answered nothing agree about nothing.
    if len(population) < QUANTITIES_AT_LEAST:
        raise SystemExit(
            f"plateforce: this request reported {len(population)} quantities, under the "
            f"{QUANTITIES_AT_LEAST} a comparison needs"
        )
    answered_for = dict(
        {arm: set(published) for arm, published in trees.items()},
        **{"folder run": {row[0] for row in folder_run_steps}},
    )
    for arm, answered in sorted(answered_for.items()):
        missing = [name for name in population if name not in answered]
        if missing:
            raise SystemExit(
                f"plateforce: the terminal reports {missing} and the {arm} arm publishes no "
                "chain for them"
            )
        extra = sorted(answered - set(population))
        if extra:
            raise SystemExit(
                f"plateforce: the {arm} arm publishes a chain for {extra}, which this request "
                "does not report"
            )

    # The shape first: which rule sits where, under which number. The two tree-publishing arms
    # are compared in the order each walks its own tree, which is more than a set comparison
    # asks; the folder run publishes a relation and is compared as one.
    shapes = {
        arm: {name: steps_of(published[name], name) for name in population}
        for arm, published in trees.items()
    }
    steps = sum(len(rows) for rows in shapes["r"].values())
    deepest = max(row[1] for rows in shapes["r"].values() for row in rows)
    print(
        f"steps compared: {steps} over {len(population)} quantities, deepest {deepest}, "
        f"across {len(answered_for)} arms"
    )
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
    against_the_relation = {row for rows in shapes["r"].values() for row in rows}
    if against_the_relation != folder_run_steps:
        raise SystemExit(
            "plateforce: the folder run stands a rule at a depth the tree does not, or the "
            "other way about:\n    only the tree "
            + str(sorted(against_the_relation - folder_run_steps))
            + "\n    only the folder run "
            + str(sorted(folder_run_steps - against_the_relation))
        )

    # Then what each step says produced its number. Numbers and named alternatives are kept
    # apart between the two arms that publish them apart, and taken together against the
    # relation, whose one column cannot tell them apart.
    for field in ("parameters", "choices"):
        told_apart = {
            arm: {
                row
                for name in population
                for row in parameters_of(published[name], name, fields=(field,))
            }
            for arm, published in trees.items()
        }
        if told_apart["r"] != told_apart["python"]:
            raise SystemExit(
                f"plateforce: one response, two accounts of which {field} produced a number:"
                "\n    only r      "
                + str(sorted(told_apart["r"] - told_apart["python"]))
                + "\n    only python "
                + str(sorted(told_apart["python"] - told_apart["r"]))
            )

    named = dict(
        {
            arm: {
                row
                for name in population
                for row in parameters_of(
                    published[name], name, fields=("parameters", "choices")
                )
            }
            for arm, published in trees.items()
        },
        **{"folder run": folder_run_named},
    )
    on_every_arm = set.intersection(*named.values())
    differences = {(arm, *row) for arm, rows in named.items() for row in rows - on_every_arm}
    print(
        f"records of what produced a number: {len(set.union(*named.values()))} across "
        f"{len(named)} arms, {len(on_every_arm)} on every one"
    )
    if not on_every_arm:
        raise SystemExit(
            "plateforce: no record appears on every arm, so the comparison below is between "
            "empty sets"
        )

    appeared = differences - KNOWN_DIFFERENCES
    repaired = KNOWN_DIFFERENCES - differences
    if appeared:
        # Named from both sides. With three arms, the surface a record is missing from is the
        # one a reader needs and the one a list of the arms that carry it does not state.
        told = [
            f"{row}\n        named by {sorted(arm for arm in named if row in named[arm])}, "
            f"missing from {sorted(arm for arm in named if row not in named[arm])}"
            for row in sorted({row[1:] for row in appeared})
        ]
        raise SystemExit(
            "plateforce: a surface names a choice behind a number that the others do not, "
            "and nothing records it:\n    " + "\n    ".join(told)
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
