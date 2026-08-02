#' @include measured.R
NULL

# Dropping the provenance is something a caller does on purpose, by naming `@value`.
#
# Leaving these methods undefined would not prevent it. R's default coercion walks the
# object underneath and returns a number: `as.numeric(list(1.23))` is `1.23` with no
# warning at all. An undefined method in R is a silent success rather than a refusal.
#
# `as.numeric` and `as.double` are one internal generic, so the method below answers both.

DROPPED_PROVENANCE <- paste(
  "a measured value carries the method that produced it.",
  "@value is the number."
)

refuse_bare_number <- function() {
  refuse_here("provenance_dropped", DROPPED_PROVENANCE, parameter = "value")
}

#' @exportS3Method base::as.double
`as.double.plateforce::measured` <- function(x, ...) refuse_bare_number()

#' @exportS3Method base::Ops
`Ops.plateforce::measured` <- function(e1, e2) refuse_bare_number()
