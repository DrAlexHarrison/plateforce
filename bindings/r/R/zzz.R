#' @include refusal.R
NULL

# Where the registry comes from, in the order a caller would expect: what they named,
# what their environment names, then the copy inside this package. The chosen path and
# its digest travel in every result, so a caller who pointed at their own copy has that
# fact in the record rather than in their memory.

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
        "no registry was named and this installation carries none.",
        "Name one with the path argument or with PLATEFORCE_REGISTRY."
      ),
      parameter = "path"
    )
  }
  shipped
}

# A registry is read once per path per session. Reading it costs a read and a validation
# of every file in the tree, and an analysis reads no rule out of it, so reading it on
# every call would put that cost on every number. Both facts a request carries come from
# the one read.
read_registries <- new.env(parent = emptyenv())

registry_facts <- function(path = NULL) {
  root <- registry_root(path)
  if (is.null(read_registries[[root]])) {
    registry <- pf_registry(root)
    read_registries[[root]] <- list(
      digest = registry@digest,
      method_ids = registry@method_ids
    )
  }
  read_registries[[root]]
}

registry_digest <- function(path = NULL) registry_facts(path)$digest

# Which of the rules a caller named this registry carries. The engine is told rather than
# asked: it reports a rule as registry backed only for the ids the request declares, so a
# request that names none produces a record stating that no rule came from the registry.
registry_backed_among <- function(method_ids, path = NULL) {
  named <- method_ids[!vapply(method_ids, is.null, logical(1))]
  if (!length(named)) {
    return(NULL)
  }
  backed <- intersect(as.character(named), registry_facts(path)$method_ids)
  if (!length(backed)) NULL else as.list(backed)
}
