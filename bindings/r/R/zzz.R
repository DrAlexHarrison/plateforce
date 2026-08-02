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

# The digest of a registry is measured once per path per session. Measuring it costs a
# read and a validation of every file in the tree, and an analysis reads no rule out of
# it, so measuring it on every call would put that cost on every number.
measured_digests <- new.env(parent = emptyenv())

registry_digest <- function(path = NULL) {
  root <- registry_root(path)
  if (is.null(measured_digests[[root]])) {
    measured_digests[[root]] <- pf_registry(root)@digest
  }
  measured_digests[[root]]
}
