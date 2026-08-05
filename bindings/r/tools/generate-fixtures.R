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

# The renderer the suite compares against lives beside the suite, so a chain written here and
# a chain read there cannot be two spellings of one record.
source(file.path(package_root, "tests", "testthat", "helper-plateforce.R"))

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
  paste("samples_matching_the_convention",
        trial@read_report[["samples_matching_the_convention"]]),
  paste("samples_carrying_no_number", trial@read_report[["samples_carrying_no_number"]])
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

# The same trial under a second onset rule, so the suite holds the comparison the quality
# signal is taken over to the numbers rather than to a description of them.
second <- analyse_countermovement_jump(
  trial,
  weighing = "bwepoch.fixed_window",
  onset = "onset.threshold.last_within_band",
  takeoff = "takeoff.threshold.absolute_force",
  registry = file.path(repository, "registry")
)

routes <- function(analysed) {
  takeoff <- analysed@values[["jump_height_from_takeoff_meters"]]@value
  flight <- analysed@values[["jump_height_from_flight_time_meters"]]@value
  c(takeoff = takeoff, flight = flight, percent = 100 * abs(takeoff - flight) / flight)
}

first_routes <- routes(result)
second_routes <- routes(second)
write_lines("disagreement.txt", c(
  paste("first_rule", "onset.threshold.noise_relative"),
  paste("first_signal_count", length(result@signals)),
  paste("first_takeoff_meters", format(first_routes[["takeoff"]], digits = 17)),
  paste("first_flight_meters", format(first_routes[["flight"]], digits = 17)),
  paste("first_percent", format(first_routes[["percent"]], digits = 17)),
  paste("second_rule", "onset.threshold.last_within_band"),
  paste("second_signal_count", length(second@signals)),
  paste("second_takeoff_meters", format(second_routes[["takeoff"]], digits = 17)),
  paste("second_flight_meters", format(second_routes[["flight"]], digits = 17)),
  paste("second_percent", format(second_routes[["percent"]], digits = 17))
))

# The sweep over the onset rules the build runs, on the one trial. The founding claim is
# that the choice of rule moves the number, so the size of that movement is recorded rather
# than described.
sweep <- pf_spread(
  trial,
  quantity = "jump_height_from_takeoff_meters",
  slot = "onset",
  weighing = "bwepoch.fixed_window",
  onset = "onset.threshold.noise_relative",
  takeoff = "takeoff.threshold.absolute_force",
  registry = file.path(repository, "registry")
)
swept <- vapply(sweep[["variants"]], function(one) {
  if (is.null(one[["value"]])) NA_real_ else as.double(one[["value"]])
}, numeric(1))
# Which rule produced which number, rather than the four summary figures alone. A fixture
# holding only the summary certifies the arithmetic and says nothing about the record, which
# is the half of this product that the arithmetic is in service of.
rules <- vapply(sweep[["variants"]], function(one) one[["settings"]][[1]][[2]], character(1))
write_lines("onset-sweep.txt", c(
  paste("variants", length(swept)),
  paste("failed", sweep[["failed"]]),
  paste("spread_absolute_meters", format(sweep[["spread_absolute"]], digits = 17)),
  paste("furthest_from_median_meters",
        format(max(abs(swept - stats::median(swept, na.rm = TRUE)), na.rm = TRUE), digits = 17)),
  paste("method_ids", paste(rules, collapse = ",")),
  # `format` pads a vector to one width, so each is written on its own to keep the joined
  # line free of the spaces that padding would put either side of every comma.
  paste("values_meters",
        paste(vapply(swept, format, character(1), digits = 17), collapse = ",")),
  paste("registry_declared_version",
        if (is.null(sweep[["registry_declared_version"]])) "none" else
          sweep[["registry_declared_version"]])
))

write_lines(
  "chain.txt",
  chain_lines(result@values[["jump_height_from_takeoff_meters"]]@provenance)
)
