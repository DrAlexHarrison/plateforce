# plateforce for R

Force-plate jump kinetics where every number carries the method that produced it: the
rule, its published parameters, its citation, and the choices made upstream of it.

Ten published ways of computing one jump height disagree by a median 3.51 cm on 244 real
trials, against the 1.98 cm training effect the source study was built to detect. So the
method travels with the number rather than in a lab notebook.

## Install

```r
install.packages("plateforce")
```

Binary installations need no compiler. Source installations need cargo and rustc 1.82 or
newer; the install stops with the version it found and the version it needs rather than
with a compiler error.

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

The registry census and executable bindings are read from the installed package:

```r
pf_registry()@census
pf_bindings()[c("construct", "id")]
```

Reading a force file needs the delimiter and the force column stated.

## The same numbers as every other surface

The R package links the same compiled engine as the browser, the command line and the
Python package.

## Licence

Apache 2.0. The Rust sources bundled with the package are listed with their licences in
`system.file("AUTHORS", package = "plateforce")`.
