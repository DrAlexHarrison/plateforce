# The R surface

What the R package offers, where its names come from, and the decisions behind its shape.
Grades are load-bearing: **DECREE** is Alex's words and binding, **LEAN** is his lean,
**AI-LEAD** is a call made under delegation and is always re-raisable, **OPEN** is
undecided.

## What it offers

`pf_trial()` and `pf_read_force_file()` build a trace. `analyse_countermovement_jump()`
runs the three landmark rules. `pf_value()` reads one quantity out of the result, and the
object it returns carries the rule that produced it, the parameters that rule read, and
whether each value was stated by the caller or supplied by the rule. `pf_registry()`,
`pf_entry()` and `pf_bindings()` read the registry.

Nothing here computes. Every number comes out of `plateforce-analysis`, which the browser,
the command line and the Python package also link, so a number that differs between two
surfaces is a build that failed rather than a discrepancy to reconcile.

## Where the names come from

The result object's quantity names are the engine's, read off the response rather than
listed in R. A quantity that arrives or is renamed upstream arrives or is renamed on this
surface without an edit here.

Registry identifiers stay in their canonical dotted form as strings. Function arguments
mirror the Python surface argument for argument.

## Decisions

### The boundary carries JSON, and the trace carries doubles

**AI-LEAD.** Every entry point on the Rust side takes and returns a JSON document that is
either `{"ok": ...}` or `{"refusal": ...}`.

Neither R binding framework can raise a classed R condition carrying fields, and the R
surface owes its caller a condition with seven of them, so a refusal has to reach R as
data whichever framework is used. Making every answer data is one rule instead of two, and
it means R receives the same bytes a cross-surface comparison reads.

The force trace does not travel that way. It is thousands of doubles, JSON round-trips it
neither cheaply nor exactly, and R doubles are already IEEE 754 binary64, so the trace
crosses as a raw double vector and the Rust side holds the trial behind an external
pointer.

R writes request JSON and never reads any. The reverse direction is one compiled walk from
`serde_json::Value` to an R object, so a document is parsed once rather than twice.

### The framework is `extendr`, chosen 2026-08-01

**AI-LEAD.** `bindings/r/src/rust/src/shim.rs` is the only file that names it, and
`bindings/r/tools/framework-seam.sh` fails when a second file learns its name. Reversing
the choice edits that file and the crate manifest.

Two measurements decided it. `extendr-api` 0.9.0 declares a toolchain floor of 1.71 and
the current `savvy` declares 1.88, against the 1.84.1 the oldest CRAN check machines run,
so taking `savvy` would mean holding it at an old release permanently against a constraint
that is CRAN's rather than ours. And the error path that argued for `savvy`, carrying a
Rust `Result` across the boundary, is a thing this design does not do.

Vendor size does not discriminate: the two dependency sets vendor to within twenty
kilobytes of each other, against a 10 MB tarball ceiling.

### The result object is S7 and is reached with `@`

**AI-LEAD.** `d$takeoff_velocity` returns `takeoff_velocity_meters_per_second`'s value on a
plain data frame with no warning at all. An `@` access on an S7 object does not partial
match. A package whose premise is that silent substitutions are what is wrong with this
field's software cannot hand a user a neighbouring property under a name they typed by
mistake.

That fixes the R floor at 4.3.0, which is where R gained `@` on S7 objects, rather than at
S7's own 3.5.0.

Batch relations are data frames because a table has to be one, and their documented
accessor is `x[["name"]]`, which is exact. A guard scans the sources, the examples and the
vignettes for a `$` on one.

### `as.numeric()`, `as.double()` and arithmetic are defined, and they refuse

**AI-LEAD.** Leaving them undefined is not protection in R: `as.numeric(list(1.23))`
returns `1.23` silently, so an undefined method is a silent success rather than a refusal.
They are defined and they name `@value` as the way to ask for the number, which makes
dropping the provenance a deliberate act.

### The engine sources are copied into the package

**AI-LEAD.** The three engine crates are not on crates.io, and `cargo vendor` does not
vendor a path dependency reached through `[patch.crates-io]`, measured 2026-08-01. So an
installable source package has to carry them. `tools/sync-engine.sh` copies one commit of
`crates/` and `registry/` into the package, records the revision and a digest, and
`tools/check-engine-version.sh` fails when the copy has been edited in place or is not the
repository's current engine.

Git carries one copy of the engine, under `crates/`. The copy inside the package is a
build artifact and is not tracked, so there is no second home to drift.

The R package does not wait on the engine crates being published to crates.io.

### The package lives at `bindings/r/`

**AI-LEAD.** Every other surface lives under `crates/`, including the Python package.
An R package cannot: `R CMD build` wants `DESCRIPTION` at the package root and compiled
sources under `src/`, and cargo wants `Cargo.toml` at the crate root, so one directory
cannot be both.

`bindings/r/src/rust/Cargo.toml` carries its own `[workspace]` table, so the R tree
resolves on its own, keeps its own lockfile, and adds no member to the repository's
workspace manifest. The cost is that `cargo test --workspace` and `cargo fmt --all` at the
repository root do not reach it, which the R workflow covers.

### The toolchain floor is recorded as data

**AI-LEAD.** `tools/msrv` holds the R crate's floor and `tools/cran-rust-floor` holds the
oldest rustc on CRAN's check farm with the flavour it belongs to, the date it was read and
the source. `tools/check-floor.sh` asserts three things: the declared floor covers every
dependency, the declared floor is at or below CRAN's, and `DESCRIPTION`'s
`SystemRequirements` equals the declared floor.

CRAN publishes no machine-readable source for that number, so the script prints the age of
the reading. A floor that has risen since only makes the guard stricter; a machine older
than the reading joining the farm makes it pass falsely, and the printed age is the only
thing that surfaces that.

The floor is 1.82, and the fourth thing the script asserts is that the tree builds on it.
The dependency tree needs 1.76 once the R crate's lockfile is pinned, and the engine
sources reach 1.82 through `Option::is_none_or`, so the floor is the higher of the two.
Reading declared versions alone reported 1.76 and would have shipped a false
`SystemRequirements` claim, which is why the guard builds rather than reads.

Pinning the R crate's lockfile below what the repository's workspace resolves is what keeps
the dependency half under CRAN's number, and is why the two crates keep separate lockfiles.

### r-universe first, CRAN after

**AI-LEAD.** r-universe builds source, Windows and macOS binaries including arm64 for
packages with compiled Rust, so it is the route users install from and nobody waits on a
CRAN decision. CRAN is still worth having, because a CRAN package is what a methods
section cites and what a university lab's IT will install.
