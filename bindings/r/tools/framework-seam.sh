#!/bin/sh
# One file names the binding framework, and this fails when a second one learns its name.
#
# The seam is what makes the framework choice reversible: swapping it edits `shim.rs` and
# the manifest. A name that has spread into the dispatch or into R is a swap that has
# become a rewrite, and it spreads at the moment the framework is wired rather than later.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")

status=0
seam="shim.rs"

for file in "$package_root"/src/rust/src/*.rs; do
    [ -e "$file" ] || continue
    name=$(basename "$file")
    [ "$name" = "$seam" ] && continue
    if grep -qE 'savvy|extendr' "$file"; then
        printf '%s names the binding framework\n' "src/rust/src/$name" >&2
        status=1
    fi
done

for file in "$package_root"/R/*.R; do
    [ -e "$file" ] || continue
    if grep -qE 'savvy|extendr' "$file"; then
        printf 'R/%s names the binding framework\n' "$(basename "$file")" >&2
        status=1
    fi
done

if [ "$status" -eq 0 ]; then
    printf '%s is the only file naming the framework\n' "$seam"
fi

# The boundary cost, measured rather than asserted. One JSON serialise and one parse per
# analysed trial was argued to be acceptable, and an argued number is the kind this
# project keeps finding to be wrong.
if [ "${PLATEFORCE_SKIP_TIMING:-}" != "1" ]; then
    trace="$package_root/../../crates/plateforce-conformance/fixtures/subject01_trial1.force.txt"
    if [ ! -f "$trace" ]; then
        printf 'no recorded trial at %s, so the boundary cost is unmeasured\n' "$trace" >&2
        exit 1
    fi
    PLATEFORCE_TIMING_TRACE="$trace" Rscript -e '
if (!requireNamespace("plateforce", quietly = TRUE)) {
  cat("plateforce is not in this R library, so the boundary cost cannot be measured\n")
  quit(status = 1)
}
# A trace that produces a jump. Timing one that declines would measure the refusal path
# and report it as the cost of an analysis.
trial <- plateforce::pf_read_force_file(
  Sys.getenv("PLATEFORCE_TIMING_TRACE"),
  sample_rate_hz = 1200, delimiter = "\t", force_column = 0, sentinel_convention = "none"
)
once <- function() {
  plateforce::analyse_countermovement_jump(
    trial,
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
}
first <- once()
if (is.na(plateforce::pf_value(first, "jump_height_from_takeoff_meters")@value)) {
  cat("the trial produced no jump height, so the cost measured is not an analysis\n")
  quit(status = 1)
}
calls <- 100L
elapsed <- numeric(calls)
for (index in seq_len(calls)) {
  # Progress is printed so a run that stops partway says where, rather than only that it
  # stopped.
  if (index %% 25L == 0L) cat("calls completed:", index, "\n")
  elapsed[index] <- system.time(once())[["elapsed"]]
}
cat(sprintf("median milliseconds per call: %.3f\n", stats::median(elapsed) * 1000))
' || status=1
fi

exit "$status"
