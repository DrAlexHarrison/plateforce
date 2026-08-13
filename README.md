# plateforce

Reads a force trace and computes jump kinetics. Every value it returns carries the rule that
produced it, that rule's parameters, and that rule's citation.

## Why that matters

Across 244 countermovement jump trials, 10 published jump-height methods disagree by a median of
3.51 cm within a single trial. The training intervention those trials were collected to measure
moved jump height by 1.98 cm. So a jump height is only comparable with another one computed the
same way, and the record of how it was computed is the thing that makes the comparison possible.

## Running it

The browser build reads the file in the tab and sends nothing anywhere:
[dralexharrison.github.io/plateforce](https://dralexharrison.github.io/plateforce)

`docs/install.md` covers the desktop application and the command line, on Linux, macOS,
Windows, and machines that will not let you install anything. `docs/terminal.md` covers
working at a terminal, and `docs/for-an-agent.md` is the contract for a program driving one.

At a terminal:

```
plateforce analyse <trace> --column 0 --sample-rate-hz 1200 --sentinel none \
  --weighing bwepoch.fixed_window --set weighing.duration=1.0 \
  --onset onset.threshold.noise_relative --set onset.k=5 \
  --takeoff takeoff.threshold.absolute_force --set takeoff.threshold_n=20
```

Leave the method flags out and it computes nothing. It names each choice that is open and what
the literature publishes for it, then exits 64:

```
plateforce: 2 of 3 choices on the path to a jump height have no default.

  --weighing <METHOD>   Standing still, before the jump   system_weight
      System weight includes the bar. Bodyweight does not. The registry records
      real conflations.
      bwepoch.fixed_window                                accepted
          duration published at 0.1, 0.25, 0.4, 0.5, 1.0, 2.0
      bwepoch.adaptive_lowest_variance                    recommended
          window_seconds published at 0.2, 0.5, 1.0, 2.0
      bwepoch.manual_placement                            accepted
```

## What it can do

```
plateforce capability        the operations, methods, output formats and refusal codes this
                             surface reaches, as JSON
plateforce registry census   the registry's populations, each counted and reported on its own
```

## Layout

```
registry/   method definitions as data: rule, citation, status, bias, parameters
crates/     the Rust workspace: registry, core maths, CLI, conformance, wasm, Python
bindings/   the R package
web/        the browser interface that the wasm build drives
docs/       method rulings, schema, and the reasoning behind both
audit/      headline_audit.py recomputes the spread above from the trial matrix, with its
            denominator
```

## Contributing

`CONVENTIONS.md` is binding and it is short.

Adding a method means adding a registry entry, not writing code.

## Citing

Harrison, A. plateforce. https://github.com/DrAlexHarrison/plateforce

`plateforce version` prints the version to cite alongside it.

## Licence

Apache-2.0. See `LICENSE` and `NOTICE`.
