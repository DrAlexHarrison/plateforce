# plateforce

Force-plate kinetic analysis with a method registry.

Every computed quantity is bound to a named method variant carrying its citation,
its exact rule, its known bias and a status flag. Choosing a different variant is
an explicit act that appears in the output, not an invisible default.

## Why the registry exists

Two independent open-source implementations of the same named methods, run over 243 of
the same 244 countermovement jump trials, agree at r = 0.961 on jump height and
**r = 0.696 on time to takeoff**.

Across those 244 trials, 9 published onset rules and 10 published jump-height methods:

| quantity | spread across published methods, within a single trial |
|---|---|
| time to takeoff | median 0.335 s, which is 38.9% of that trial's own value |
| jump height | median 3.51 cm |

For reference, the training intervention that dataset was collected to measure moved
jump height by 1.98 cm. The method choice moves the number further than the training did.

Agreement is also not the whole story, because the disagreement is not evenly spread.
**Two of the nine published onset rules place the start of the movement more than two
seconds before takeoff on roughly one trial in seven**, on a movement lasting under a
second. Their median behaviour is unremarkable and their 95th percentile is three times
normal, so a single reported bias figure for those rules averages "agrees reasonably"
together with "found the wrong event entirely".

Every figure on this page is recomputed by `audit/headline_audit.py` rather than quoted,
and each one states the query and the denominator that produced it. The underlying trial
matrix is not yet released: the 2011 corpus it derives from is re-identifiable and the
consent position for the full cohort is unresolved, so the analysis is open and the data
release is pending rather than declined.

## Status

Early, and the browser build runs: **https://dralexharrison.github.io/plateforce/**

Drop a force trace in and nothing is uploaded. The file is read in the tab.

What works: the registry loads and validates as data, 246 computation entries and 8
protocol entries across 18 method files. The core reproduces a frozen 56-variant
reference implementation over 244 trials, every column, and a difference fails the build.

What that does not yet mean: **12 of the registry's rules are executable**, covering the
weighing epoch, movement onset and takeoff. The rest of the registry is catalogued and
cited but has no running maths behind it yet, and the interface says so per rule rather
than quietly substituting a default. There is no desktop application, the command line
inspects the registry but does not analyse a trace, and the R binding does not exist.

## Layout

```
registry/   method definitions as data: rule, citation, status, bias, parameters
crates/     the Rust workspace: registry, core maths, CLI, conformance, wasm, python
web/        the browser interface that the wasm build drives
docs/       method rulings, schema, and the reasoning behind both
audit/      the script that recomputes every number quoted in this README
```

## Contributing

Read `CONVENTIONS.md` first. It is binding, and it is short.

Adding a method means adding a registry entry, not writing code.

## Licence

Apache-2.0. See `LICENSE` and `NOTICE`.
