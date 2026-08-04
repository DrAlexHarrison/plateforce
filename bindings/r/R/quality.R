#' What the software knows about a number it is handing back
#'
#' A signal compares two values this result already carries and states an action. It is not
#' a refusal: the number stands, and the reader decides what to do about the comparison.
#'
#' `@remedy_construct` names the construct whose rule the reader would change, rather than a
#' single rule id, so the published alternatives are one call away.
#'
#' @noRd
quality_signal <- S7::new_class(
  "quality_signal",
  package = "plateforce",
  properties = list(
    label = S7::class_character,
    value = S7::class_double,
    unit = S7::class_character,
    threshold = S7::class_double,
    status = S7::class_character,
    remedy = S7::class_character,
    remedy_construct = S7::class_character,
    qualifies = S7::class_character
  )
)

#' @export
`print.plateforce::quality_signal` <- function(x, ...) {
  # A signal holding no value says which status it is under, read off the record rather than
  # spelled again here, so a status this package has never heard of still reaches the reader
  # under its own name. The unit and the threshold belong to a comparison that produced a
  # number, and both stay on the object for a caller who wants them.
  figure <- if (is.na(x@value)) {
    gsub("_", " ", x@status, fixed = TRUE)
  } else {
    sprintf(
      "%s %s, threshold %s %s",
      format(x@value, digits = 4), x@unit, format(x@threshold, digits = 4), x@unit
    )
  }
  cat(sprintf("%s: %s\n", x@label, figure))
  cat(sprintf("  %s\n", x@remedy))
  invisible(x)
}

signal_from_list <- function(fields) {
  quality_signal(
    label = as.character(fields[["label"]]),
    value = if (is.null(fields[["value"]])) NA_real_ else as.double(fields[["value"]]),
    unit = as.character(fields[["unit"]]),
    threshold = as.double(fields[["threshold"]]),
    status = as.character(fields[["status"]]),
    remedy = as.character(fields[["remedy"]]),
    remedy_construct = as.character(fields[["remedy_construct"]]),
    qualifies = as.character(unlist(fields[["qualifies"]]))
  )
}

#' The signals raised over one analysis
#'
#' Each names what was compared, the value it compared, the threshold it applied, and an
#' action. A result with none is a result nothing was noticed about.
#'
#' @param x A `countermovement_jump`.
#' @return A list of signals.
#' @export
#' @examples
#' standing <- pf_trial(rep(700, 1200), sample_rate_hz = 1200)
#' result <- analyse_countermovement_jump(
#'   standing,
#'   weighing = "bwepoch.fixed_window",
#'   onset = "onset.threshold.noise_relative",
#'   takeoff = "takeoff.threshold.absolute_force"
#' )
#' pf_signals(result)
pf_signals <- function(x) x@signals
