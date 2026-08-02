#' What the plate and its settings were
#'
#' Five facts about the capture that no reanalysis recovers. A block missing any of them
#' fingerprints as incomplete, so results from that trial are never declared to match
#' another lab's.
#'
#' `sample_rate_hz` is the sixth member of the block and belongs to the trial, so it is
#' stated in [pf_trial()] rather than here.
#'
#' @param filter_at_capture The filter the plate applied before the trace was written,
#'   which no later filtering undoes.
#' @param tare_state Whether and when the plate was zeroed.
#' @param plate_natural_frequency_hz The plate's natural frequency.
#' @param floor_surface What the plate was standing on.
#' @param firmware_version The plate firmware that wrote the trace.
#' @return A named list, for [pf_trial()] and [pf_read_force_file()].
#' @export
#' @examples
#' partial <- pf_acquisition(tare_state = "tared_before_trial")
#' trial <- pf_trial(rep(700, 1200), sample_rate_hz = 1200, acquisition = partial)
#' trial@acquisition_complete
#' trial@acquisition_missing
pf_acquisition <- function(filter_at_capture = NULL,
                           tare_state = NULL,
                           plate_natural_frequency_hz = NULL,
                           floor_surface = NULL,
                           firmware_version = NULL) {
  stated <- list(
    filter_at_capture = filter_at_capture,
    tare_state = tare_state,
    plate_natural_frequency_hz = plate_natural_frequency_hz,
    floor_surface = floor_surface,
    firmware_version = firmware_version
  )
  stated[!vapply(stated, is.null, logical(1))]
}

#' The members the acquisition block declares
#'
#' Named by the block itself, so a member that arrives upstream arrives here.
#'
#' @return A character vector of member names.
#' @export
#' @examples
#' pf_acquisition_members()
pf_acquisition_members <- function() {
  as.character(unlist(unwrap(decode(rust_acquisition_members_json()))))
}
