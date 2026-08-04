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

## `synthetic_untrimmed_step_off_after_jump.force.txt`

The same pillar from the other end, and the one recording in this directory on which the two
landmarks come back in the wrong order. Synthetic, and named so. One column of vertical force
at 1200 Hz: 1.2 s of quiet standing, a countermovement jump with its flight and its landing, a
settle, then the athlete steps off the plate and the recording keeps running for two more
seconds. No athlete produced it.

The flight is not a number anybody typed. `audit/build_untrimmed_step_off_after_jump_fixture.py`
builds the trace up to takeoff, solves its propulsive peak for a takeoff velocity of 2.30 m/s by
bisection, integrates the net impulse over system weight, and gives the flight exactly the 2v/g
that impulse buys. So jump height from the impulse and from the flight time both read 26.97 cm
on it, rather than one being asserted beside the other. Propulsive peak 3.08 system weights,
landing peak 4.00, countermovement velocity minimum 0.88 m/s downward, time to takeoff 0.67 s.

Because the plate reads emptiest after the athlete steps off rather than during the flight,
`onset.threshold.adaptive_trailing_window`, whose entry says it keeps the last departure from
quiet before the force extremum, places the start of the jump at 3.5433 s against a takeoff of
1.8567 s. Swept at their shipped defaults, 8 of the 50 combinations of 2 weighing rules, 5 onset
rules and 5 takeoff rules invert the landmarks here; the corpus produces that on 0 of the 12,300
combinations it offers.

## `landing_shape_placements_subject01.tsv`

Where the reference implementation of the landing-shape rule placed takeoff on subject 01's
six trials, as `subject`, `trial`, the weighing figure it used, the sample index, and how many
landings it found. An empty index is a recording that closes no run with a landing.

Only subject 01 is ever public, so the same placements for the rest of the corpus are named
by `PLATEFORCE_REFERENCE_PLACEMENTS` rather than committed.
