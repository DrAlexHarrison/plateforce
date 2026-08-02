# plateforce for R

Force-plate jump kinetics where every number carries the method that produced it: the
rule, its published parameters, its citation, and the choices made upstream of it.

Ten published ways of computing one jump height disagree by 3.51 cm on 244 real trials,
against the 1.98 cm training effect the source study was built to detect. So the method
travels with the number rather than in a lab notebook.

## Install

```r
install.packages("plateforce", repos = c(
  "https://dralexharrison.r-universe.dev",
  "https://cloud.r-project.org"
))
```

Windows and macOS get a binary and need no compiler. On Linux the package builds from
source and needs cargo and rustc 1.82 or newer; the install stops with the version it
found and the version it needs rather than with a compiler error.

## Use

```r
library(plateforce)

trial <- pf_read_force_file(
  "trial.txt",
  sample_rate_hz = 1200,
  delimiter = "\t",
  force_column = 2
)

result <- analyse_countermovement_jump(
  trial,
  weighing = "bwepoch.fixed_window",
  onset    = "onset.threshold.noise_relative",
  takeoff  = "takeoff.threshold.absolute_force"
)

height <- pf_value(result, "jump_height_from_takeoff_meters")
height@value
height@provenance@depends_on[[1]]@parameters
```

`vignette("plateforce")` is the one-minute path.

## What it covers

The registry enumerates every method entry and every construct, and the count is a query
rather than a line in this file:

```r
pf_registry()@census
```

Three of those constructs have executable rules behind them on this surface: the weighing
epoch that establishes system weight, movement onset, and takeoff. The quantities derived
from them are system weight and mass, onset and takeoff time, time to takeoff, flight
time, takeoff velocity, net impulse, jump height by the takeoff-velocity route and by the
flight-time route, and modified reactive strength index.

Reading a force file needs the delimiter and the force column stated. Batch analysis over
a directory, Parquet and Arrow output, and the method-spread sweep are on the other
surfaces of this project and are not reachable from R.

## The same numbers as every other surface

The R package links the same compiled engine as the browser, the command line and the
Python package. It computes nothing of its own, so a number that differs between two
surfaces is a build that failed rather than a discrepancy to reconcile.

## Licence

Apache 2.0. The Rust sources bundled with the package are listed with their licences in
`system.file("AUTHORS", package = "plateforce")`.
