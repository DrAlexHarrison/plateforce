#' @include request.R rust.R
NULL

#' Refusals
#'
#' A rule that declines raises a condition rather than returning a sentinel. The condition
#' carries the code, the method, the slot, the parameter, the value, the detail and what
#' was available instead, so a caller branches on the fields rather than parsing a
#' sentence.
#'
#' The class vector is `c("plateforce_<code>", "plateforce_refusal", "plateforce_error")`,
#' so `tryCatch` handles one code or every refusal.
#'
#' Read the fields with `[[`. `cnd$method` returns the value of `method_id` on a plain
#' condition, silently, because `$` partial matches on a list, so this package defines
#' `$` on a refusal to require the whole name.
#'
#' @name plateforce_refusal
#' @examples
#' standing <- pf_trial(rep(700, 1200), sample_rate_hz = 1200)
#' condition <- tryCatch(
#'   analyse_countermovement_jump(
#'     standing,
#'     weighing = "bwepoch.fixed_window",
#'     onset = "onset.not.a.rule",
#'     takeoff = "takeoff.threshold.absolute_force"
#'   ),
#'   plateforce_refusal = identity
#' )
#' condition[["code"]]
#' condition[["message"]]
NULL

REFUSAL_FIELDS <- c(
  "code", "message", "method_id", "slot",
  "parameter", "value", "named_value", "detail", "available"
)

#' A refusal as the condition object, built rather than raised.
#'
#' One rule declining while the rest of the analysis computes is a partial result, not a
#' failed one, so those arrive on `@refusals` and are read with the same `[[` as a caught
#' condition.
#'
#' @noRd
refusal_condition <- function(refusal) {
  errorCondition(
    message = refusal[["message"]],
    class = c(
      paste0("plateforce_", refusal[["code"]]),
      "plateforce_refusal",
      "plateforce_error"
    ),
    code = refusal[["code"]],
    method_id = refusal[["method_id"]],
    slot = refusal[["slot"]],
    parameter = refusal[["parameter"]],
    value = refusal[["value"]],
    named_value = refusal[["named_value"]],
    detail = refusal[["detail"]],
    available = refusal[["available"]]
  )
}

plateforce_refuse <- function(refusal) {
  stop(refusal_condition(refusal))
}

#' @export
`$.plateforce_refusal` <- function(x, name) {
  fields <- names(unclass(x))
  if (!name %in% fields) {
    stop(errorCondition(
      message = paste0(
        "a refusal is read by whole field name: ", deparse(substitute(x)),
        "[[\"", name, "\"]]. This one carries ", paste(fields, collapse = ", "), "."
      ),
      class = c(
        "plateforce_field_not_named_in_full",
        "plateforce_refusal",
        "plateforce_error"
      ),
      code = "field_not_named_in_full",
      parameter = name,
      available = fields
    ))
  }
  unclass(x)[[name]]
}

# Everything this package receives from the engine arrives as one of two shapes, and this
# is the only place either is opened.
unwrap <- function(envelope) {
  if (!is.null(envelope[["refusal"]])) {
    plateforce_refuse(envelope[["refusal"]])
  }
  envelope[["ok"]]
}

refuse_here <- function(code, message, ...) {
  fields <- list(code = code, message = message, ...)
  for (name in REFUSAL_FIELDS) {
    if (is.null(fields[[name]])) fields[[name]] <- NULL
  }
  plateforce_refuse(fields)
}
