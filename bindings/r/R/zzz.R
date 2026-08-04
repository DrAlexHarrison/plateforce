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
      declared_version = registry@declared_version,
      method_ids = registry@method_ids
    )
  }
  read_registries[[root]]
}

registry_digest <- function(path = NULL) registry_facts(path)$digest

# The revision the registry names about itself, or NULL where it names none. A separate
# question from the revision a caller pins, and the request carries both: one is what the
# data claims and the other is what the author cited, and a reader handed one field for
# both is told the author chose a revision nobody chose.
registry_declared_version <- function(path = NULL) {
  declared <- registry_facts(path)$declared_version
  if (!length(declared)) NULL else as.character(declared)
}

# What this registry carries. The engine is told rather than asked, and it judges every rule
# it binds against this list, so the list is every id rather than the ones a caller named:
# the binding composes operators onto the rule the caller chose, and each of those is an
# entry in its own right. A list built from the caller's choices alone reports a published
# entry as absent from the registry it is filed in.
registry_backed_ids <- function(path = NULL) {
  ids <- registry_facts(path)$method_ids
  if (!length(ids)) NULL else as.list(as.character(ids))
}
