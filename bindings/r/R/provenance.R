#' What produced a number
#'
#' The rule that ran, what it was bound to, where each bound value came from, which
#' registry it was read out of, and the chain of values this one rests on.
#'
#' `registry_version` is the revision the caller pinned and is empty when they pinned none.
#' `registry_declared_version` is the revision the registry names about itself. They are
#' separate because a reader handed one field for both is told the author cited a revision
#' the data named for itself.
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
    registry_declared_version = S7::class_character,
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
provenance_from_bound_method <- function(bound, stamp, acquisition_complete) {
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
    registry_version = stamp$version,
    registry_declared_version = stamp$declared_version,
    registry_digest = stamp$digest,
    acquisition_complete = acquisition_complete,
    depends_on = list()
  )
}

# What the response says about the registry behind it, as the three character vectors the
# provenance class holds. Read once per response so no record on it can answer differently,
# and empty rather than NA where the response says nothing: a revision nobody pinned is a
# fact about the request, not a value this session failed to read.
registry_stamp_of <- function(response) {
  said <- function(name) {
    value <- response[[name]]
    if (is.null(value)) character(0) else as.character(value)
  }
  list(
    version = said("registry_version"),
    declared_version = said("registry_declared_version"),
    digest = said("registry_digest")
  )
}
