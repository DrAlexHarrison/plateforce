# Requests are written here and read by the engine. R writes JSON and never reads it.
#
# The shape rules are stated rather than inferred. A named list is an object, an unnamed
# list is an array, a length-one atomic vector is a scalar, and any other atomic vector is
# an array. `NULL` is omitted by the caller rather than written as null.

as_json <- function(value, name = "") {
  if (is.null(value)) {
    return("null")
  }
  if (is.list(value)) {
    if (!length(value)) {
      return(if (is.null(names(value))) "[]" else "{}")
    }
    if (is.null(names(value))) {
      return(paste0(
        "[",
        paste(vapply(value, as_json, character(1), name = name), collapse = ","),
        "]"
      ))
    }
    pairs <- vapply(seq_along(value), function(index) {
      field <- names(value)[index]
      paste0(as_json_string(field), ":", as_json(value[[index]], name = field))
    }, character(1))
    return(paste0("{", paste(pairs, collapse = ","), "}"))
  }
  scalars <- vapply(value, as_json_atom, character(1), name = name)
  if (length(scalars) == 1L) scalars else paste0("[", paste(scalars, collapse = ","), "]")
}

as_json_atom <- function(value, name) {
  # `NaN`, `NA_real_` and the infinities, each of which used to leave here as a value the
  # engine never met: the first two as `null`, which reads as a value nobody stated, and the
  # infinities as bare `Inf`, which is not JSON and came back naming a column of the document
  # rather than the parameter. A number the caller typed is refused under its own name.
  if (is.numeric(value) && (is.na(value) || !is.finite(value))) {
    refuse_here(
      "parameter_not_finite",
      paste0(name, " must be a finite number, got ", format(value)),
      parameter = name
    )
  }
  if (is.na(value)) {
    return("null")
  }
  if (is.logical(value)) {
    return(if (value) "true" else "false")
  }
  if (is.character(value)) {
    return(as_json_string(value))
  }
  # Seventeen significant digits round-trip a binary64 through text without widening it.
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
