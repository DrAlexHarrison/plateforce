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
if [ "${PLATEFORCE_SKIP_TIMING:-}" != "1" ] && command -v Rscript >/dev/null 2>&1; then
    Rscript -e '
if (!requireNamespace("plateforce", quietly = TRUE)) {
  cat("median milliseconds per call: the package is not installed in this library\n")
  quit(status = 0)
}
trial <- plateforce::pf_trial(rep(700, 2400), sample_rate_hz = 1200)
once <- function() {
  tryCatch(
    plateforce::analyse_countermovement_jump(
      trial,
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    ),
    plateforce_refusal = function(condition) condition
  )
}
elapsed <- vapply(seq_len(100), function(i) system.time(once())[["elapsed"]], numeric(1))
cat(sprintf("median milliseconds per call: %.3f\n", stats::median(elapsed) * 1000))
' 2>/dev/null || printf 'median milliseconds per call: not measured in this library\n'
fi

exit "$status"
