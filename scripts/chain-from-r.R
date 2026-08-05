# R's tree behind every number of one analysis, as JSON on this stream.
#
# The trace is read from the path in the first argument and the request goes through
# `analyse_countermovement_jump`, which is the call a user's own session makes, so what this
# prints is what an R session holds rather than a second record assembled here.

suppressPackageStartupMessages(library(plateforce))

trace <- commandArgs(trailingOnly = TRUE)[1]
if (is.na(trace)) stop("the trace to read is the first argument")

analysed <- analyse_countermovement_jump(
  pf_read_force_file(trace, sample_rate_hz = 1200, delimiter = "\t", force_column = 0,
                     sentinel_convention = "none"),
  weighing = "bwepoch.fixed_window",
  onset = "onset.threshold.noise_relative",
  takeoff = "takeoff.threshold.absolute_force",
  weighing_parameters = list(duration = 1.0),
  onset_parameters = list(k = 5.0),
  takeoff_parameters = list(threshold_n = 20.0)
)

# The class this package hands a caller, walked into the two fields the comparison reads. The
# parameter names alone: the comparison asks which choices a surface says produced a number.
as_tree <- function(provenance) {
  list(
    method_id = provenance@method_id,
    parameters = as.list(sort(provenance@parameters[["name"]])),
    depends_on = unname(lapply(provenance@depends_on, as_tree))
  )
}

trees <- list()
for (quantity in names(analysed@values)) {
  trees[[quantity]] <- as_tree(pf_value(analysed, quantity)@provenance)
}

# `auto_unbox` on for the scalars and the parameter lists left as lists, so a step carrying one
# name is a list of one on this stream rather than a bare string the reader would iterate
# character by character.
cat(jsonlite::toJSON(trees, auto_unbox = TRUE, null = "null", pretty = FALSE))
