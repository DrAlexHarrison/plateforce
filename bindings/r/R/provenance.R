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

# A record's list of name, value and source, as the data frame the provenance class holds.
binding_frame_from <- function(records) {
  binding_frame(
    vapply(records, function(record) as.character(record[["name"]]), character(1)),
    vapply(records, function(record) as.character(record[["value"]]), character(1)),
    vapply(records, function(record) as.character(record[["source"]]), character(1))
  )
}

# Empty rather than NA where the record says nothing: a revision nobody pinned is not a value
# this session failed to read.
said_in <- function(record, name) {
  value <- record[[name]]
  if (is.null(value)) character(0) else as.character(value)
}

# The record the engine wrote, as the class this package hands a caller.
#
# Read rather than rebuilt. The chain behind one number is derived in
# `plateforce_analysis::chain_of` and reaches here whole, so what an R session holds is what a
# folder run and a notebook hold. This package used to assemble the chain from `bound_methods`
# and `contributing_method_ids`, which put every contributing rule at one depth under a root
# carrying none of the arithmetic's own values, and recorded every named choice as a parameter.
provenance_from_record <- function(record) {
  provenance(
    method_id = said_in(record, "method_id"),
    parameters = binding_frame_from(record[["parameters"]]),
    choices = binding_frame_from(record[["choices"]]),
    registry_version = said_in(record, "registry_version"),
    registry_declared_version = said_in(record, "registry_declared_version"),
    registry_digest = said_in(record, "registry_digest"),
    acquisition_complete = isTRUE(record[["acquisition_complete"]]),
    depends_on = lapply(record[["depends_on"]], provenance_from_record)
  )
}

# What the response says about the registry behind it, as the three character vectors the
# provenance class holds. Empty rather than NA where the response says nothing: a revision
# nobody pinned is not a value this session failed to read.
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
