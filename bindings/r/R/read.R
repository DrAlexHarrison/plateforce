#' @include trial.R request.R
NULL

#' Read a force file
#'
#' The file is read by the engine. Nothing in this package parses it, so there is one
#' answer to which column carries force and one answer to what a row means.
#'
#' Every choice the read rests on is stated by the caller and reported back. A rate that
#' is guessed scales every velocity, displacement, impulse and rate of force development
#' with it, and a column that is guessed can be the wrong one quietly, so neither has a
#' default.
#'
#' @param path Path to a delimited text export.
#' @param sample_rate_hz The rate the trace was recorded at.
#' @param delimiter The single character separating this file's columns.
#' @param force_column Zero-based index of the column carrying vertical ground reaction
#'   force.
#' @param sentinel_convention How this export writes a missing sample: `"none"`, `"zero"`
#'   or `"negative_one"`.
#' @return A `trial` whose `@read_report` names the delimiter, the column, the rows read,
#'   the columns per row, the blank lines skipped, and the samples the sentinel convention
#'   treated as missing.
#' @export
pf_read_force_file <- function(path,
                               sample_rate_hz = NULL,
                               delimiter = NULL,
                               force_column = NULL,
                               sentinel_convention = "none") {
  carried <- rust_trial_from_file(request_of(
    path = path,
    sample_rate_hz = sample_rate_hz,
    delimiter = delimiter,
    force_column = if (is.null(force_column)) NULL else as.integer(force_column),
    sentinel_convention = sentinel_convention
  ))
  trial_from_carried(carried)
}
