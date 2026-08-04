#' @include analyse.R
NULL

#' How far the method choice moves one number
#'
#' Sweeps a slot's alternatives on one trial and reports the spread over them. This is the
#' question the whole registry exists to answer, so it sits beside
#' [analyse_countermovement_jump()] rather than behind a switch, and it takes no option to
#' enable it.
#'
#' @param trial A [trial].
#' @param quantity The engine's name for the quantity to sweep, for example
#'   `"jump_height_from_takeoff_meters"`.
#' @param slot The slot whose alternatives are swept: `"weighing"`, `"onset"` or
#'   `"takeoff"`.
#' @param method_ids Registry identifiers to sweep. When absent, every rule this build
#'   runs for that slot, read off the binding table rather than listed here.
#' @param parameter A parameter name to sweep instead of the method.
#' @param values Values for that parameter.
#' @param maximum_combinations Cap on how many combinations run.
#' @inheritParams analyse_countermovement_jump
#' @return A list carrying `variants`, one per combination with its label, its value and
#'   the reason it declined when it did, and the summary the sweep computed over them:
#'   `minimum`, `maximum`, `median`, `spread_absolute`, `spread_percent_of_median`, and
#'   `succeeded` and `failed` beside the count that was requested.
#' @export
#' @examples
#' standing <- pf_trial(rep(700, 1200), sample_rate_hz = 1200)
#' sweep <- pf_spread(
#'   standing,
#'   quantity = "system_weight_newtons",
#'   slot = "weighing",
#'   weighing = "bwepoch.fixed_window",
#'   onset = "onset.threshold.noise_relative",
#'   takeoff = "takeoff.threshold.absolute_force"
#' )
#' sweep[["succeeded"]]
#' sweep[["spread_absolute"]]
pf_spread <- function(trial,
                      quantity,
                      slot,
                      weighing,
                      onset,
                      takeoff,
                      method_ids = NULL,
                      parameter = NULL,
                      values = NULL,
                      gravity_meters_per_second_squared = NULL,
                      weighing_parameters = NULL,
                      onset_parameters = NULL,
                      takeoff_parameters = NULL,
                      maximum_combinations = NULL,
                      registry = NULL) {
  if (is.null(method_ids) && is.null(parameter)) {
    bindings <- pf_bindings()
    method_ids <- bindings[["id"]][bindings[["slot"]] == slot]
    if (!length(method_ids)) {
      refuse_here(
        "slot_has_no_rules",
        paste0("this build runs rules for ", paste(unique(bindings[["slot"]]),
                                                   collapse = ", ")),
        slot = slot,
        available = unique(bindings[["slot"]])
      )
    }
  }

  axis <- drop_empty(list(
    slot = slot,
    parameter = parameter,
    values = if (is.null(values)) NULL else as.double(values),
    method_ids = if (is.null(method_ids)) NULL else as.list(as.character(method_ids))
  ))

  request <- request_of(
    base = drop_empty(list(
      weighing = drop_empty(list(
        method_id = weighing,
        parameters = weighing_parameters
      )),
      onset = drop_empty(list(method_id = onset, parameters = onset_parameters)),
      takeoff = drop_empty(list(method_id = takeoff, parameters = takeoff_parameters)),
      gravity_meters_per_second_squared = gravity_meters_per_second_squared,
      gravity_source = gravity_claim(gravity_meters_per_second_squared)
    )),
    axes = list(axis),
    quantity_key = quantity,
    maximum_combinations = if (is.null(maximum_combinations)) {
      NULL
    } else {
      as.integer(maximum_combinations)
    }
  )

  unwrap(decode(rust_spread_json(trial@handle, request)))
}
