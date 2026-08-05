#' @include analyse.R
NULL

#' How far the method choice moves one number
#'
#' Sweeps a step's alternatives on one trial and reports the spread over them.
#'
#' @param trial A [trial].
#' @param quantity The engine's name for the quantity to sweep, for example
#'   `"jump_height_from_takeoff_meters"`.
#' @param slot One step whose rule is swept, or several. Several sweeps every combination of
#'   them, which is the question a number resting on more than one rule asks: the onset rule
#'   and the takeoff rule both move a jump height, and sweeping them one at a time reports
#'   neither the widest disagreement nor the narrowest.
#' @param method_ids Registry identifiers to sweep. A character vector is the set for the one
#'   step named; a list keyed by step names the ids for each. Absent, every rule bound to
#'   that step is compared, read off the binding table rather than listed here.
#' @param vary Settings to sweep, keyed by the step and the setting, as
#'   `list("onset.k" = c(2, 5, 10))`. Numbers or names per key: `"epoch_impulse.convention" =
#'   c("net", "gross")` compares two names, and `"global.gravity_meters_per_second_squared"`
#'   sweeps the value the run carries rather than one a rule reads. Written beside `slot`, the
#'   rules and the settings vary together.
#' @param maximum_combinations Cap on how many combinations run.
#' @inheritParams analyse_countermovement_jump
#' @return A list carrying `variants`, one per combination with its label, its value and
#'   the reason it declined when it did, and the summary the sweep computed over them:
#'   `minimum`, `maximum`, `median`, `spread_absolute`, `spread_percent_of_median`, and
#'   `succeeded` and `failed` beside the count that was requested. A sweep over one
#'   combination carries no `spread_absolute`, because one number holds no disagreement.
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
#'
#' # The rules and a value inside them, on one call.
#' rules_and_a_value <- pf_spread(
#'   standing,
#'   quantity = "system_weight_newtons",
#'   slot = "weighing",
#'   vary = list("weighing.duration" = c(0.5, 1.0)),
#'   weighing = "bwepoch.fixed_window",
#'   onset = "onset.threshold.noise_relative",
#'   takeoff = "takeoff.threshold.absolute_force"
#' )
#' rules_and_a_value[["combinations_run"]]
pf_spread <- function(trial,
                      quantity,
                      slot = NULL,
                      weighing = NULL,
                      onset = NULL,
                      takeoff = NULL,
                      preset = NULL,
                      method_ids = NULL,
                      vary = NULL,
                      gravity_meters_per_second_squared = NULL,
                      body_mass_kilograms = NULL,
                      weighing_parameters = NULL,
                      onset_parameters = NULL,
                      takeoff_parameters = NULL,
                      weighing_options = NULL,
                      onset_options = NULL,
                      takeoff_options = NULL,
                      weighing_start_index = NULL,
                      onset_index = NULL,
                      takeoff_index = NULL,
                      touchdown_index = NULL,
                      derived = NULL,
                      conditioning = NULL,
                      maximum_combinations = NULL,
                      registry = NULL,
                      registry_version = NULL) {
  request <- spread_request_of(
    quantity = quantity, slot = slot, method_ids = method_ids, vary = vary,
    weighing = weighing, onset = onset, takeoff = takeoff, derived = derived,
    conditioning = conditioning,
    gravity_meters_per_second_squared = gravity_meters_per_second_squared,
    body_mass_kilograms = body_mass_kilograms,
    weighing_parameters = weighing_parameters, onset_parameters = onset_parameters,
    takeoff_parameters = takeoff_parameters,
    weighing_options = weighing_options, onset_options = onset_options,
    takeoff_options = takeoff_options,
    weighing_start_index = weighing_start_index, onset_index = onset_index,
    takeoff_index = takeoff_index, touchdown_index = touchdown_index,
    maximum_combinations = maximum_combinations,
    registry = registry, registry_version = registry_version
  )

  # The pipeline is laid on by the engine rather than here, as the analysis lays it on.
  reply <- if (is.null(preset)) {
    rust_spread_json(trial@handle, request)
  } else {
    rust_spread_under_preset_json(trial@handle, registry_root(registry), preset, request)
  }
  unwrap(decode(reply))
}

# The one place a sweep request is written.
#
# The base goes through the builder the analysis writes, so the combination that varies
# nothing is the request `analyse_countermovement_jump` sends and the sweep is around the
# caller's own result rather than around one assembled here. The parity arm asks through this
# too: an arm that wrote its own request would send a document nobody sends, and the gate
# would be measuring that arm.
spread_request_of <- function(quantity, slot = NULL, method_ids = NULL, vary = NULL,
                              weighing = NULL, onset = NULL, takeoff = NULL,
                              derived = NULL, conditioning = NULL,
                              gravity_meters_per_second_squared = NULL,
                              body_mass_kilograms = NULL,
                              weighing_parameters = NULL, onset_parameters = NULL,
                              takeoff_parameters = NULL,
                              weighing_options = NULL, onset_options = NULL,
                              takeoff_options = NULL,
                              weighing_start_index = NULL, onset_index = NULL,
                              takeoff_index = NULL, touchdown_index = NULL,
                              maximum_combinations = NULL,
                              registry = NULL, registry_version = NULL) {
  do.call(request_of, c(
    list(
      base = analysis_fields_of(
        weighing = weighing, onset = onset, takeoff = takeoff, derived = derived,
        conditioning = conditioning,
        gravity_meters_per_second_squared = gravity_meters_per_second_squared,
        body_mass_kilograms = body_mass_kilograms,
        weighing_parameters = weighing_parameters, onset_parameters = onset_parameters,
        takeoff_parameters = takeoff_parameters,
        weighing_options = weighing_options, onset_options = onset_options,
        takeoff_options = takeoff_options,
        weighing_start_index = weighing_start_index, onset_index = onset_index,
        takeoff_index = takeoff_index, touchdown_index = touchdown_index,
        registry = registry
      ),
      axes = axes_of(slot, method_ids, vary),
      quantity_key = quantity,
      maximum_combinations = if (is.null(maximum_combinations)) {
        NULL
      } else {
        as.integer(maximum_combinations)
      }
    ),
    registry_identity_of(registry, registry_version)
  ))
}

# The dimensions to sweep: the steps whose rule varies, and the settings whose value does.
#
# Both at once, which is the sweep the engine has always run and no surface could ask for.
# Five onset rules by three values of `k` is `slot = "onset", vary = list("onset.k" =
# c(2, 5, 10))`, and the terminal writes the same request as `--slot onset --vary
# onset.k=2,5,10`.
#
# One step is one axis. Named twice it was two, and the sweep squared its own combinations,
# each one binding the step twice and the second binding winning, so the denominator every
# figure is reported over counts a set the caller never asked for. The terminal and the
# notebook refuse the repeat in the words below, and this refuses it in the same ones.
#
# A character vector reaches the wire as a JSON array, so several names in `slot` used to
# leave here inside one axis and come back as a parse fault naming a column of the request.
axes_of <- function(slot, method_ids, vary) {
  named <- if (is.null(slot)) character() else as.character(slot)
  repeated <- named[duplicated(named)]
  if (length(repeated)) {
    refuse_here(
      "value_not_accepted",
      paste0("'", repeated[[1]], "' is named twice, and one step is one axis"),
      slot = repeated[[1]],
      available = named
    )
  }

  listed <- rules_named(method_ids, named)
  axes <- lapply(named, function(step) axis_of(step, listed[[step]]))
  for (step in setdiff(names(listed), named)) {
    axes <- c(axes, list(axis_of(step, listed[[step]])))
  }
  axes <- c(axes, settings_varied(vary))

  if (!length(axes)) {
    refuse_here(
      "required_parameter_unstated",
      "no step and no setting were named, so there is nothing to sweep",
      parameter = "slot"
    )
  }
  written <- vapply(axes, function(axis) {
    if (is.null(axis[["parameter"]])) axis[["slot"]] else
      paste0(axis[["slot"]], ".", axis[["parameter"]])
  }, character(1))
  repeated <- written[duplicated(written)]
  if (length(repeated)) {
    refuse_here(
      "value_not_accepted",
      paste0("'", repeated[[1]], "' is named twice, and one setting is one axis"),
      parameter = repeated[[1]],
      available = written
    )
  }
  axes
}

# The rules to compare, per step, where the caller named them rather than taking every rule
# the build runs.
#
# A character vector is the set for the one step named, which is the shape a folder run's
# `--against` takes. A list keyed by step says which ids belong to which, so named rules on
# two steps is one call: as a vector that question cannot be asked, because the vector cannot
# say which step each id is for.
rules_named <- function(method_ids, named) {
  if (is.null(method_ids)) {
    return(list())
  }
  if (is.list(method_ids)) {
    if (is.null(names(method_ids)) || any(!nzchar(names(method_ids)))) {
      refuse_here(
        "value_not_accepted",
        "a list of method_ids is keyed by step, as list(onset = c(\"a\", \"b\"))",
        parameter = "method_ids"
      )
    }
    return(lapply(method_ids, as.character))
  }
  if (length(named) != 1) {
    refuse_here(
      "value_not_accepted",
      "a vector of method_ids describes one step, so name one step or key the ids by step",
      parameter = "method_ids",
      available = named
    )
  }
  structure(list(as.character(method_ids)), names = named)
}

# One setting to sweep per key, written as the terminal writes it: `"onset.k"`, and
# `"global.gravity_meters_per_second_squared"` for the value the run carries rather than a
# rule.
#
# Numbers or names per key and never both. A name is a choice in the sense a number is, so
# `list("epoch_impulse.convention" = c("net", "gross"))` is a sweep, and an axis carrying one
# of each has no width between them.
settings_varied <- function(vary) {
  if (is.null(vary) || !length(vary)) {
    return(list())
  }
  if (is.null(names(vary)) || any(!nzchar(names(vary)))) {
    refuse_here(
      "value_not_accepted",
      "vary is keyed by the step and the setting, as list(\"onset.k\" = c(2, 5, 10))",
      parameter = "vary"
    )
  }
  lapply(names(vary), function(qualified) {
    parts <- regmatches(qualified, regexpr(".", qualified, fixed = TRUE), invert = TRUE)[[1]]
    if (length(parts) != 2 || !nzchar(parts[[1]])) {
      refuse_here(
        "value_not_accepted",
        paste0("'", qualified, "' names no step, and vary is keyed by the step and the setting"),
        parameter = "vary"
      )
    }
    alternatives <- vary[[qualified]]
    if (!length(alternatives)) {
      refuse_here(
        "required_parameter_unstated",
        paste0("no value was named for ", qualified, ", so there is nothing to sweep"),
        parameter = qualified
      )
    }
    repeated <- alternatives[duplicated(alternatives)]
    if (length(repeated)) {
      refuse_here(
        "value_not_accepted",
        paste0(qualified, " names ", repeated[[1]], " twice, and one value is one variant"),
        parameter = qualified
      )
    }
    # Both lists rather than vectors. A length-one atomic vector is written as a scalar, so one
    # value to sweep left here as a number where the engine reads a list of them, and came back
    # naming a column of the request rather than the value the caller typed.
    if (is.character(alternatives)) {
      list(slot = parts[[1]], parameter = parts[[2]],
           options = as.list(as.character(alternatives)))
    } else {
      list(slot = parts[[1]], parameter = parts[[2]],
           values = as.list(as.double(alternatives)))
    }
  })
}

# A step named on its own is swept over the rules the binding table holds for it, and a step
# the table holds one rule for has no alternative for that rule to be compared against. The
# terminal and the notebook refuse that in the sentence below.
#
# A list of ids the caller wrote is the set they mean, one long or five, and is not held to
# that floor: naming a bound rule against itself is one variant and runs. An empty list names
# nothing at all.
axis_of <- function(slot, method_ids) {
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

  list(slot = slot, method_ids = as.list(as.character(method_ids)))
}
