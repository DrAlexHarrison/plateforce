# Conformance fixture

Six trials and the six frozen reference rows that go with them. `cargo test` runs the
comparison against these on every machine, and a difference fails the build.

`subject01_trialN.force.txt` is one vertical ground reaction force value per line, in
newtons, at 1200 Hz. It is column index 2 of the original tab separated export, copied
across as text so no value was parsed and reprinted on the way.

`reference_subject01.csv` is the six matching rows of the frozen reference output, which
is a numpy implementation of 56 method variants whose rules are each cited to the
upstream file and line they reproduce.

## What may be here

One subject, addressed by number. The remaining 40 subjects of the source corpus are not
redistributable and are not here. Source directory names are athlete names, so nothing
derived from a path component may reach these files; the trace files contain digits,
signs and decimal points and nothing else.

`audit/build_conformance_fixture.py` rebuilds these from a local corpus and refuses any
other subject.

## The full run

The evidence behind the project is the run over all 244 trials, which happens where the
corpus is. Point the gated test at it:

    PLATEFORCE_CONFORMANCE_CORPUS=<corpus root> \
    PLATEFORCE_CONFORMANCE_REFERENCE=<reference csv> \
    cargo test -p plateforce-conformance

or get the per column table from `plateforce-conformance --reference ... --corpus ...`.

## `synthetic_untrimmed_step_off.force.txt`

Synthetic, and named so. One column of vertical force at 1200 Hz: a second of quiet standing,
a step off the plate and back on, a settle, then a countermovement jump with its flight and
its landing. No athlete produced it.

It exists because `MISSION.md` P5 names an untrimmed recording as a pillar's own test and the
corpus cannot supply one. Surveyed across the 242 trials the shipped rules place a takeoff
on, 0 return a time to takeoff under 150 ms, because every corpus recording was trimmed to a
single jump before it was archived.

On this trace the shipped takeoff rules place takeoff on the step-off and report 58.3 ms;
`takeoff.threshold.landing_shape` places it on the jump and reports 2269.2 ms.

## `landing_shape_placements_subject01.tsv`

Where the reference implementation of the landing-shape rule placed takeoff on subject 01's
six trials, as `subject`, `trial`, the weighing figure it used, the sample index, and how many
landings it found. An empty index is a recording that closes no run with a landing.

Only subject 01 is ever public, so the same placements for the rest of the corpus are named
by `PLATEFORCE_REFERENCE_PLACEMENTS` rather than committed.
