#' @include provenance.R
NULL

#' A number and the method that produced it
#'
#' `@value` is the number, `@unit` is the unit the registry spells it in, and
#' `@provenance` is the rule that produced it with everything it was bound to.
#'
#' Properties are reached with `@`, which requires the whole name. A shortened name is an
#' error rather than a neighbouring property's value.
#'
#' @noRd
measured <- S7::new_class(
  "measured",
  package = "plateforce",
  properties = list(
    value = S7::class_double,
    unit = S7::class_character,
    unit_symbol = S7::class_character,
    quantity = S7::class_character,
    provenance = provenance,
    account = S7::class_character
  )
)

#' @export
`print.plateforce::measured` <- function(x, ...) {
  method <- x@provenance@method_id
  cat(sprintf(
    "%s %s  %s\n",
    format(x@value, digits = 15),
    x@unit_symbol,
    x@quantity
  ))
  cat(sprintf(
    "%s\n",
    if (length(method)) method else "no registry entry names this arithmetic"
  ))
  invisible(x)
}

measured_from_list <- function(fields) {
  known <- c("value", "unit", "unit_symbol", "quantity", "provenance", "account")
  unknown <- setdiff(names(fields), known)
  if (length(unknown)) {
    refuse_here(
      "unknown_field",
      paste0(
        "this record carries ", paste(unknown, collapse = ", "),
        "; a measured value carries ", paste(known, collapse = ", ")
      ),
      parameter = unknown[1],
      available = known
    )
  }
  measured(
    value = as.double(fields[["value"]]),
    unit = as.character(fields[["unit"]]),
    unit_symbol = as.character(fields[["unit_symbol"]]),
    quantity = as.character(fields[["quantity"]]),
    provenance = fields[["provenance"]],
    account = as.character(fields[["account"]])
  )
}
