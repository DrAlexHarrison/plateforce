#' @include refusal.R rust.R
NULL

#' The version of this package
#'
#' @return A single string, the package version.
#' @export
#' @examples
#' pf_version()
pf_version <- function() {
  unwrap(decode(rust_version_json()))[["package_version"]]
}
