#' @include refusal.R
NULL

#' One force trace
#'
#' `@read_report` records what the reader did: the sentinel convention it applied, how many
#' samples matched that convention, how many carried no number at all, and for a file, the
#' delimiter, the force column and the rows it read.
#'
#' The two counts are separate because they are two facts. On a jump trace the zero
#' convention a vendor writes for a missing measurement is also the correct reading of an
#' unloaded plate, so it matches the whole flight phase, and a reader told only the total
#' cannot tell a gap in the recording from the athlete being in the air.
#'
#' @noRd
trial <- S7::new_class(
  "trial",
  package = "plateforce",
  properties = list(
    sample_count = S7::class_integer,
    sample_rate_hz = S7::class_double,
    duration_seconds = S7::class_double,
    read_report = S7::class_list,
    acquisition_complete = S7::class_logical,
    acquisition_missing = S7::class_character,
    handle = S7::class_any
  )
)

#' @export
`print.plateforce::trial` <- function(x, ...) {
  cat(sprintf(
    "%d samples at %g Hz, %g s\n",
    x@sample_count, x@sample_rate_hz, x@duration_seconds
  ))
  cat(sprintf(
    "sentinel convention %s, %d samples matching it, %d samples carrying no number\n",
    x@read_report[["sentinel_convention"]],
    x@read_report[["samples_matching_the_convention"]],
    x@read_report[["samples_carrying_no_number"]]
  ))
  invisible(x)
}

#' Build a trial from a force vector
#'
#' @param force_newtons Vertical ground reaction force, in newtons, as a `double` vector.
#' @param sample_rate_hz The rate the trace was recorded at. A rate that is guessed scales
#'   every velocity, displacement, impulse and rate of force development with it, so this
#'   has no default.
#' @param sentinel_convention How this export writes a missing sample: `"none"`, `"zero"`
#'   or `"negative_one"`. A sample matching the convention is counted and left where it is,
#'   because closing the gap would shift every timestamp after it. Samples carrying no
#'   number are counted separately, whatever convention is declared.
#' @param acquisition What the plate and its settings were, from [pf_acquisition()]. A
#'   block missing any member makes every result from this trial carry
#'   `acquisition_complete = FALSE`.
#' @param plate A saved plate to fill the block from: a name this machine holds, or the list
#'   [pf_plate()] returns. A member in `acquisition` beside it is the answer that runs, and
#'   the result records what it replaced.
#' @param plates_folder Where to look for `plate`, when it is a name. Absent reads the folder
#'   `plateforce plate save` writes to.
#' @return A `trial`.
#' @export
#' @examples
#' quiet <- pf_trial(rep(700, 1200), sample_rate_hz = 1200)
#' quiet@sample_count
#' quiet@duration_seconds
pf_trial <- function(force_newtons, sample_rate_hz = NULL, sentinel_convention = "none",
                     acquisition = NULL, plate = NULL, plates_folder = NULL) {
  if (is.integer(force_newtons)) {
    refuse_here(
      "force_not_double",
      paste(
        "a force trace is a double vector. Widen it with as.double(), so the widening",
        "is an act this record can carry."
      ),
      parameter = "force_newtons"
    )
  }
  if (!is.double(force_newtons)) {
    refuse_here(
      "force_not_double",
      "a force trace is a double vector",
      parameter = "force_newtons",
      value = class(force_newtons)[1]
    )
  }
  carried <- rust_trial_from_force(
    force_newtons,
    request_of(
      sample_rate_hz = sample_rate_hz,
      sentinel_convention = sentinel_convention,
      acquisition = acquisition,
      plate = stated_plate_of(plate, plates_folder)
    )
  )
  trial_from_carried(carried)
}

trial_from_carried <- function(carried) {
  report <- unwrap(decode(carried[["envelope"]]))
  trial(
    sample_count = as.integer(report[["sample_count"]]),
    sample_rate_hz = as.double(report[["sample_rate_hz"]]),
    duration_seconds = as.double(report[["duration_seconds"]]),
    read_report = report,
    acquisition_complete = isTRUE(report[["acquisition_complete"]]),
    acquisition_missing = as.character(unlist(report[["acquisition_missing"]])),
    handle = carried[["handle"]]
  )
}

#' The force trace a trial holds
#'
#' @param x A `trial`.
#' @return A `double` vector of vertical ground reaction force in newtons.
#' @export
#' @examples
#' quiet <- pf_trial(rep(700, 12), sample_rate_hz = 12)
#' length(pf_force(quiet))
pf_force <- function(x) {
  rust_trial_force(x@handle)
}
