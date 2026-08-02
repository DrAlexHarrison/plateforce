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

# The engine records a source per name as the rule reads it, and that record is taken
# here rather than worked back out. Deriving it from a second field is a second answer to
# where a value came from, and the two say different things the moment either moves: a
# value the caller typed, one the rule fell back to, and one it measured off this trace
# move the number identically.
provenance_from_bound_method <- function(bound, registry_digest, acquisition_complete) {
  recorded <- bound[["parameter_sources"]]
  pairs <- bound[["bound_parameters"]]
  names <- vapply(pairs, function(pair) as.character(pair[[1]]), character(1))
  values <- vapply(pairs, function(pair) as.character(pair[[2]]), character(1))
  sources <- vapply(names, function(name) {
    said <- recorded[[name]]
    if (is.null(said)) {
      refuse_here(
        "parameter_source_unrecorded",
        "this rule's record does not say where the value came from",
        method_id = as.character(bound[["method_id"]]),
        parameter = name
      )
    }
    as.character(said)
  }, character(1), USE.NAMES = FALSE)

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
