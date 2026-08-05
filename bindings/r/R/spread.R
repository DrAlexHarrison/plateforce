#' @include analyse.R
NULL

#' How far the method choice moves one number
#'
#' Sweeps a slot's alternatives on one trial and reports the spread over them.
#'
#' @param trial A [trial].
#' @param quantity The engine's name for the quantity to sweep, for example
#'   `"jump_height_from_takeoff_meters"`.
#' @param slot One step whose alternatives are swept, or several. Several sweeps every
#'   combination of them, which is the question a number resting on more than one rule asks:
#'   the onset rule and the takeoff rule both move a jump height, and sweeping them one at a
#'   time reports neither the widest disagreement nor the narrowest.
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
#'
#' # Several steps at once, over every combination of their rules.
#' over_the_landmarks <- pf_spread(
#'   standing,
#'   quantity = "system_weight_newtons",
#'   slot = c("weighing", "onset"),
#'   weighing = "bwepoch.fixed_window",
#'   onset = "onset.threshold.noise_relative",
#'   takeoff = "takeoff.threshold.absolute_force"
#' )
#' over_the_landmarks[["combinations_run"]]
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
                      body_mass_kilograms = NULL,
                      weighing_parameters = NULL,
                      onset_parameters = NULL,
                      takeoff_parameters = NULL,
                      maximum_combinations = NULL,
                      registry = NULL) {
  axes <- axes_of(slot, method_ids, parameter, values)

  request <- request_of(
    base = drop_empty(list(
      weighing = drop_empty(list(
        method_id = weighing,
        parameters = weighing_parameters
      )),
      onset = drop_empty(list(method_id = onset, parameters = onset_parameters)),
      takeoff = drop_empty(list(method_id = takeoff, parameters = takeoff_parameters)),
      gravity_meters_per_second_squared = gravity_meters_per_second_squared,
      gravity_source = gravity_claim(gravity_meters_per_second_squared),
      body_mass_kilograms = body_mass_kilograms
    )),
    axes = axes,
    quantity_key = quantity,
    maximum_combinations = if (is.null(maximum_combinations)) {
      NULL
    } else {
      as.integer(maximum_combinations)
    },
    # Measured from the registry this call loaded, as `analyse_countermovement_jump` does,
    # so the sweep names the registry it read.
    registry_digest = registry_digest(registry),
    # What the registry claims about itself, read from the same registry the digest was
    # measured over. A separate question from the pin above.
    registry_declared_version = registry_declared_version(registry)
  )

  unwrap(decode(rust_spread_json(trial@handle, request)))
}

# The dimensions to sweep, one per step named.
#
# One step is one axis. Named twice it was two, and the sweep squared its own combinations,
# each one binding the step twice and the second binding winning, so the denominator every
# figure is reported over counts a set the caller never asked for. The terminal and the
# notebook refuse the repeat in the words below, and this refuses it in the same ones.
#
# A character vector reaches the wire as a JSON array, so several names in `slot` used to
# leave here inside one axis and come back as a parse fault naming a column of the request.
axes_of <- function(slot, method_ids, parameter, values) {
  named <- as.character(slot)
  if (!length(named)) {
    refuse_here(
      "required_parameter_unstated",
      "no step was named, so there is nothing to sweep",
      parameter = "slot"
    )
  }
  if (length(named) > 1 && (!is.null(parameter) || !is.null(method_ids))) {
    refuse_here(
      "sweep_axes_not_understood",
      "parameter and method_ids each describe one step, so name one step or neither",
      parameter = if (is.null(parameter)) "method_ids" else "parameter",
      available = named
    )
  }
  repeated <- named[duplicated(named)]
  if (length(repeated)) {
    refuse_here(
      "sweep_axes_not_understood",
      paste0("'", repeated[[1]], "' is named twice, and one step is one axis"),
      slot = repeated[[1]],
      available = named
    )
  }
  lapply(named, axis_of, method_ids = method_ids, parameter = parameter, values = values)
}

# A step named on its own is swept over the rules the binding table holds for it, and a step
# the table holds one rule for has no alternative for that rule to be compared against. The
# terminal and the notebook refuse that in the sentence below.
#
# A list of ids the caller wrote is the set they mean, one long or five, and is not held to
# that floor: naming a bound rule against itself is one variant and runs. An empty list names
# nothing at all.
axis_of <- function(slot, method_ids, parameter, values) {
  if (is.null(parameter)) {
    if (is.null(method_ids)) {
      bindings <- pf_bindings()
      method_ids <- bindings[["id"]][bindings[["slot"]] == slot]
      if (length(method_ids) < 2) {
        runs <- if (!length(method_ids)) "no rule" else "one rule"
        refuse_here(
          "slot_offers_no_alternative",
          paste0("this analysis runs ", runs, " for ", slot,
                 ", so there is nothing to sweep"),
          slot = slot,
          available = unique(bindings[["slot"]])
        )
      }
    } else if (!length(method_ids)) {
      refuse_here(
        "slot_offers_no_alternative",
        paste0("no rule was named for ", slot, ", so there is nothing to sweep"),
        slot = slot
      )
    }
  }

  # Both lists rather than vectors. A length-one atomic vector is written as a scalar, so one
  # value to sweep left here as a number where the engine reads a list of them, and came back
  # naming a column of the request rather than the value the caller typed.
  drop_empty(list(
    slot = slot,
    parameter = parameter,
    values = if (is.null(values)) NULL else as.list(as.double(values)),
    method_ids = if (is.null(method_ids)) NULL else as.list(as.character(method_ids))
  ))
}
