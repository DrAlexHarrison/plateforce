#' @include refusal.R
NULL

# Where the registry comes from, in the order a caller would expect: what they named, what
# their environment names, then the copy inside this package. The chosen path and its digest
# travel in every result.

registry_root <- function(path = NULL) {
  if (!is.null(path)) {
    return(normalizePath(path, mustWork = FALSE))
  }
  named <- Sys.getenv("PLATEFORCE_REGISTRY", unset = "")
  if (nzchar(named)) {
    return(normalizePath(named, mustWork = FALSE))
  }
  shipped <- system.file("registry", package = "plateforce")
  if (!nzchar(shipped)) {
    refuse_here(
      "registry_not_found",
      paste(
        "the registry directory is absent.",
        "Name the directory with the path argument or with PLATEFORCE_REGISTRY."
      ),
      parameter = "path"
    )
  }
  shipped
}

# A registry is read once per path per session: reading it validates every file in the tree,
# and an analysis reads no rule out of it.
read_registries <- new.env(parent = emptyenv())

registry_facts <- function(path = NULL) {
  root <- registry_root(path)
  if (is.null(read_registries[[root]])) {
    registry <- pf_registry(root)
    read_registries[[root]] <- list(
      digest = registry@digest,
      declared_version = registry@declared_version,
      method_ids = registry@method_ids
    )
  }
  read_registries[[root]]
}

registry_digest <- function(path = NULL) registry_facts(path)$digest

# The revision the registry names about itself, or NULL where it names none. A separate
# question from the revision a caller pins, and the request carries both.
registry_declared_version <- function(path = NULL) {
  declared <- registry_facts(path)$declared_version
  if (!length(declared)) NULL else as.character(declared)
}

# Every id this registry carries, rather than the ones a caller named: a binding composes
# operators onto the rule the caller chose, and each of those is an entry in its own right.
registry_backed_ids <- function(path = NULL) {
  ids <- registry_facts(path)$method_ids
  if (!length(ids)) NULL else as.list(as.character(ids))
}
