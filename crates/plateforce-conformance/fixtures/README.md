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
