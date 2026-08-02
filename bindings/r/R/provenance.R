#' What produced a number
#'
#' The rule that ran, what it was bound to, where each bound value came from, which
#' registry it was read out of, and the chain of values this one rests on.
#'
#' `parameters` and `choices` are data frames with columns `name`, `value` and `source`.
#' A record per parameter is what carries `source`, and `source` is one of `stated`,
#' `assumed`, `measured` or `provisional`.
#'
#' Read a column with `x[["name"]]`. A `$` on a data frame matches a shortened name
#' without saying so, and these column names are long on purpose.
#'
#' @noRd
provenance <- S7::new_class(
  "provenance",
  package = "plateforce",
  properties = list(
    method_id = S7::class_character,
    parameters = S7::class_data.frame,
    choices = S7::class_data.frame,
    registry_version = S7::class_character,
    registry_digest = S7::class_character,
    acquisition_complete = S7::class_logical,
    depends_on = S7::class_list
  )
)

EMPTY_BINDING <- function() {
  data.frame(
    name = character(0),
    value = character(0),
    source = character(0),
    stringsAsFactors = FALSE
  )
}

binding_frame <- function(names, values, sources) {
  if (!length(names)) {
    return(EMPTY_BINDING())
  }
  data.frame(
    name = as.character(names),
    value = as.character(values),
    source = as.character(sources),
    stringsAsFactors = FALSE
  )
}

# The engine reports what each rule was bound to and which of those names it supplied
# itself. A name the caller stated and a name the rule chose are different facts about the
# number, and the record is where the difference has to survive.
provenance_from_bound_method <- function(bound, registry_digest, acquisition_complete) {
  assumed <- as.character(unlist(bound[["assumed_parameters"]]))
  pairs <- bound[["bound_parameters"]]
  names <- vapply(pairs, function(pair) as.character(pair[[1]]), character(1))
  values <- vapply(pairs, function(pair) as.character(pair[[2]]), character(1))
  sources <- ifelse(names %in% assumed, "assumed", "stated")

  provenance(
    method_id = bound[["method_id"]],
    parameters = binding_frame(names, values, sources),
    choices = EMPTY_BINDING(),
    registry_version = character(0),
    registry_digest = if (is.null(registry_digest)) character(0) else registry_digest,
    acquisition_complete = acquisition_complete,
    depends_on = list()
  )
}
