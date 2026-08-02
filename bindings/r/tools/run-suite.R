# Runs the suite beside the checkout, where the recorded trial and the package sources are.
#
# `R CMD check` unpacks the package on its own, away from both, so the assertions that
# recompute from that trial and the ones that scan the sources skip there. A skip reads
# exactly like a pass in a summary, so this run fails on one: an assertion that cannot
# reach what it needs has not been made.

package <- commandArgs(trailingOnly = TRUE)[1]
if (is.na(package)) package <- "bindings/r"

results <- testthat::test_local(package, reporter = "summary", stop_on_failure = FALSE)

expectations <- unlist(lapply(results, function(file) file$results), recursive = FALSE)
skipped <- Filter(function(one) inherits(one, "expectation_skip"), expectations)
failed <- Filter(function(one) inherits(one, c("expectation_failure", "expectation_error")),
                 expectations)

cat(sprintf("expectations %d, failed %d, skipped %d\n",
            length(expectations), length(failed), length(skipped)))

for (one in skipped) {
  cat("skipped:", conditionMessage(one), "\n")
}

if (length(failed)) {
  quit(status = 1)
}
if (length(skipped)) {
  cat("an assertion skipped where what it needs is present\n")
  quit(status = 1)
}
