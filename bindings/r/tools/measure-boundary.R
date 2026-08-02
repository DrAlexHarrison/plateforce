# What one analysed trial costs through this boundary, measured rather than argued.
#
# One JSON serialise and one parse per analysed trial was argued to be acceptable, and an
# argued number is the kind this project keeps finding to be wrong. The figure moves with
# the machine, so it is printed rather than compared against a threshold.

trace <- commandArgs(trailingOnly = TRUE)[1]
if (is.na(trace) || !file.exists(trace)) {
  cat("no recorded trial at", trace, "so the boundary cost is unmeasured\n")
  quit(status = 1)
}
if (!requireNamespace("plateforce", quietly = TRUE)) {
  cat("plateforce is not in this R library, so the boundary cost cannot be measured\n")
  quit(status = 1)
}

cat("reading:", trace, "\n")
trial <- plateforce::pf_read_force_file(
  trace,
  sample_rate_hz = 1200, delimiter = "\t", force_column = 0, sentinel_convention = "none"
)
cat("read:", trial@sample_count, "samples\n")

once <- function() {
  plateforce::analyse_countermovement_jump(
    trial,
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
}

# A trial that declines produces no jump height, and timing it would report the cost of a
# path that does no analysis as the cost of one that does.
first <- once()
cat("first analysis:", length(first@values), "quantities\n")
if (is.na(plateforce::pf_value(first, "jump_height_from_takeoff_meters")@value)) {
  cat("the trial produced no jump height, so the cost measured is not an analysis\n")
  quit(status = 1)
}

calls <- 100L
elapsed <- numeric(calls)
for (index in seq_len(calls)) {
  # A run that stops partway says how far it got, rather than only that it stopped.
  if (index %% 25L == 0L) cat("calls completed:", index, "\n")
  elapsed[index] <- system.time(once())[["elapsed"]]
}
cat(sprintf("median milliseconds per call: %.3f\n", stats::median(elapsed) * 1000))
