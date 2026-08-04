#' @include refusal.R zzz.R
NULL

#' The method registry
#'
#' @noRd
registry <- S7::new_class(
  "registry",
  package = "plateforce",
  properties = list(
    root = S7::class_character,
    digest = S7::class_character,
    declared_version = S7::class_character,
    census = S7::class_data.frame,
    method_ids = S7::class_character,
    construct_ids = S7::class_character,
    protocol_ids = S7::class_character,
    preset_ids = S7::class_character
  )
)

#' Load the method registry
#'
#' @param path Directory holding the registry. When absent, the registry shipped inside
#'   this package is used, unless `PLATEFORCE_REGISTRY` names another one.
#' @return A `registry`. `@census` is one row per population, with each derived count
#'   beside the denominator it was taken over. The two populations are separate counts and
#'   are never added together, so there is no total row and no total column.
#'   `@digest` identifies the files that were read, measured from their bytes.
#'   `@declared_version` is the revision the registry names about itself, empty where it
#'   names none. The two answer different questions: a revision is a name to cite and a
#'   digest says which bytes were behind it, and the revision lives beside the rules rather
#'   than among them, so a digest cannot recover it.
#' @export
#' @examples
#' reg <- pf_registry()
#' reg@census
pf_registry <- function(path = NULL) {
  root <- registry_root(path)
  report <- unwrap(decode(rust_registry_json(root)))
  rows <- report[["census"]]
  census <- data.frame(
    population = vapply(rows, function(row) row[["population"]], character(1)),
    count = vapply(rows, function(row) as.integer(row[["count"]]), integer(1)),
    genuine_debates = vapply(rows, function(row) or_missing(row[["genuine_debates"]]), integer(1)),
    can_find_wrong_event = vapply(
      rows, function(row) or_missing(row[["can_find_wrong_event"]]), integer(1)
    ),
    stringsAsFactors = FALSE
  )
  declared <- report[["declared_version"]]
  registry(
    root = report[["root"]],
    digest = report[["digest"]],
    # Empty rather than NA, so a registry naming no revision reads as naming none rather
    # than as a revision this session failed to read.
    declared_version = if (is.null(declared)) character(0) else as.character(declared),
    census = census,
    method_ids = as.character(unlist(report[["method_ids"]])),
    construct_ids = as.character(unlist(report[["construct_ids"]])),
    protocol_ids = as.character(unlist(report[["protocol_ids"]])),
    preset_ids = as.character(unlist(report[["preset_ids"]]))
  )
}

or_missing <- function(value) if (is.null(value)) NA_integer_ else as.integer(value)

#' One registry entry
#'
#' @param id Canonical dotted identifier, for example `"onset.threshold.noise_relative"`.
#' @param path Directory holding the registry, as in [pf_registry()].
#' @return A list carrying the entry's parameters, citations, biases, failure block,
#'   disagreements and surfacing verdict, as the registry states them.
#' @export
#' @examples
#' entry <- pf_entry("onset.threshold.noise_relative")
#' entry[["construct"]]
pf_entry <- function(id, path = NULL) {
  unwrap(decode(rust_registry_entry_json(registry_root(path), id)))
}

#' The rules this build can run
#'
#' @return A data frame with one row per rule, carrying the registry id it binds, the slot
#'   it fills, the construct it computes, and the entry it composes an operator onto when
#'   it does.
#' @export
#' @examples
#' pf_bindings()[["id"]]
pf_bindings <- function() {
  rows <- unwrap(decode(rust_bindings_json()))
  text <- function(field) {
    vapply(rows, function(row) {
      value <- row[[field]]
      if (is.null(value)) NA_character_ else as.character(value)
    }, character(1))
  }
  data.frame(
    id = text("id"),
    slot = text("slot"),
    construct = text("construct"),
    title = text("title"),
    composed_from = text("composed_from"),
    stringsAsFactors = FALSE
  )
}
