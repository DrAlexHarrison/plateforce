# Requests are written here and read by the engine. R writes JSON and never reads it: a
# reader in R would be a second reading of a document the compiled side already holds
# parsed, and the two would disagree about a number's precision before they disagreed
# about anything worth arguing over.
#
# The shape rules are stated rather than inferred. A named list is an object, an unnamed
# list is an array, a length-one atomic vector is a scalar, and any other atomic vector is
# an array. `NULL` is omitted by the caller rather than written as null.

as_json <- function(value) {
  if (is.null(value)) {
    return("null")
  }
  if (is.list(value)) {
    if (!length(value)) {
      return(if (is.null(names(value))) "[]" else "{}")
    }
    if (is.null(names(value))) {
      return(paste0("[", paste(vapply(value, as_json, character(1)), collapse = ","), "]"))
    }
    pairs <- vapply(seq_along(value), function(index) {
      paste0(as_json_string(names(value)[index]), ":", as_json(value[[index]]))
    }, character(1))
    return(paste0("{", paste(pairs, collapse = ","), "}"))
  }
  scalars <- vapply(value, as_json_atom, character(1))
  if (length(scalars) == 1L) scalars else paste0("[", paste(scalars, collapse = ","), "]")
}

as_json_atom <- function(value) {
  if (is.na(value)) {
    return("null")
  }
  if (is.logical(value)) {
    return(if (value) "true" else "false")
  }
  if (is.character(value)) {
    return(as_json_string(value))
  }
  # Fifteen significant digits round-trips a binary64 through text without widening it,
  # and a number written short here is a number the engine was never given.
  format(value, digits = 17, scientific = FALSE, trim = TRUE)
}

as_json_string <- function(text) {
  escaped <- gsub("\\", "\\\\", text, fixed = TRUE)
  escaped <- gsub("\"", "\\\"", escaped, fixed = TRUE)
  escaped <- gsub("\n", "\\n", escaped, fixed = TRUE)
  escaped <- gsub("\r", "\\r", escaped, fixed = TRUE)
  escaped <- gsub("\t", "\\t", escaped, fixed = TRUE)
  paste0("\"", escaped, "\"")
}

# A name the caller did not set is left out of the request rather than sent as null, so a
# rule that supplies its own value is recorded as having supplied it.
request_of <- function(...) {
  fields <- list(...)
  fields <- fields[!vapply(fields, is.null, logical(1))]
  as_json(fields)
}
