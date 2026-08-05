#' @include rust.R refusal.R
NULL

#' What this surface can be asked to do
#'
#' Every method available, the operations this package dispatches, the container formats it
#' writes, and every way it can decline with the exit status a shell reads.
#'
#' The operations are the ones this package exports rather than a list forwarded from the
#' engine.
#'
#' @return A single JSON string with sorted keys and no spacing, so a comparison against
#'   another surface is a plain diff.
#' @export
#' @examples
#' substr(capability_json(), 1, 40)
capability_json <- function() {
  rust_capability_json()
}
