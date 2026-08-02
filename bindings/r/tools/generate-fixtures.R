# Writes tests/testthat/fixtures/ by running the engine, so no expected number in the
# suite was typed by a person and every one moves when the engine moves.
#
# It runs against a trial in the repository and writes only what the engine derived from
# it. No trace is copied into the package.

here <- dirname(sub("^--file=", "", grep("^--file=", commandArgs(FALSE), value = TRUE)[1]))
if (is.na(here) || !nzchar(here)) here <- "tools"
package_root <- normalizePath(file.path(here, ".."))
repository <- normalizePath(file.path(package_root, "..", ".."))
fixtures <- file.path(package_root, "tests", "testthat", "fixtures")
dir.create(fixtures, recursive = TRUE, showWarnings = FALSE)

library(plateforce)

trace <- file.path(repository, "crates", "plateforce-conformance", "fixtures",
                   "subject01_trial1.force.txt")
if (!file.exists(trace)) {
  stop("no trial to run at ", trace, call. = FALSE)
}

trial <- pf_read_force_file(
  trace,
  sample_rate_hz = 1200,
  delimiter = "\t",
  force_column = 0,
  sentinel_convention = "none"
)

result <- analyse_countermovement_jump(
  trial,
  weighing = "bwepoch.fixed_window",
  onset = "onset.threshold.noise_relative",
  takeoff = "takeoff.threshold.absolute_force",
  registry = file.path(repository, "registry")
)

write_lines <- function(name, lines) {
  writeLines(lines, file.path(fixtures, name))
  cat(name, length(lines), "lines\n")
}

write_lines("trial.txt", c(
  paste("sample_count", trial@sample_count),
  paste("sample_rate_hz", format(trial@sample_rate_hz, digits = 17)),
  paste("duration_seconds", format(trial@duration_seconds, digits = 17)),
  paste("delimiter", trial@read_report[["delimiter"]]),
  paste("force_column", trial@read_report[["force_column"]]),
  paste("rows_read", trial@read_report[["rows_read"]]),
  paste("columns_per_row", trial@read_report[["columns_per_row"]]),
  paste("blank_lines_skipped", trial@read_report[["blank_lines_skipped"]]),
  paste("sentinel_convention", trial@read_report[["sentinel_convention"]]),
  paste("samples_treated_as_missing", trial@read_report[["samples_treated_as_missing"]])
))

quantities <- names(result@values)
write_lines("values.txt", vapply(quantities, function(name) {
  value <- result@values[[name]]
  paste(name, format(value@value, digits = 17), value@unit, value@unit_symbol)
}, character(1)))

write_lines("landmarks.txt", c(
  paste("weighing_start_index", result@weighing_start_index),
  paste("weighing_end_index", result@weighing_end_index),
  paste("onset_index", result@onset_index),
  paste("takeoff_index", result@takeoff_index),
  paste("touchdown_index", result@touchdown_index)
))

# The same trial under an onset rule whose two routes to a height disagree, so the suite
# holds the signal to the numbers that raise it rather than to the fact that it fired.
disagreeing <- analyse_countermovement_jump(
  trial,
  weighing = "bwepoch.fixed_window",
  onset = "onset.threshold.last_within_band",
  takeoff = "takeoff.threshold.absolute_force",
  registry = file.path(repository, "registry")
)

raised <- disagreeing@signals[[1]]
write_lines("disagreement.txt", c(
  paste("agreeing_signal_count", length(result@signals)),
  paste("signal_count", length(disagreeing@signals)),
  paste("status", raised@status),
  paste("value", format(raised@value, digits = 17)),
  paste("threshold", format(raised@threshold, digits = 17)),
  paste("unit", raised@unit),
  paste("remedy_construct", raised@remedy_construct),
  paste("qualifies", paste(raised@qualifies, collapse = " ")),
  paste("jump_height_from_takeoff_meters",
        format(disagreeing@values[["jump_height_from_takeoff_meters"]]@value, digits = 17)),
  paste("jump_height_from_flight_time_meters",
        format(disagreeing@values[["jump_height_from_flight_time_meters"]]@value, digits = 17))
))

chain <- result@values[["jump_height_from_takeoff_meters"]]@provenance@depends_on
write_lines("chain.txt", vapply(chain, function(step) {
  bound <- step@parameters
  paste(
    step@method_id,
    paste(paste0(bound[["name"]], "=", bound[["value"]], "(", bound[["source"]], ")"),
          collapse = " ")
  )
}, character(1)))
