#' @include measured.R provenance.R trial.R
NULL

#' One countermovement jump, analysed
#'
#' `@values` is a named list of `measured` objects, one per quantity the engine reported,
#' keyed by the engine's own name for it. Read one with `x@values[["name"]]`, which
#' matches the whole name, or with [pf_value()].
#'
#' The names come from the engine rather than from a list written here, so a quantity added
#' or renamed upstream carries through without an edit.
#'
#' @noRd
countermovement_jump <- S7::new_class(
  "countermovement_jump",
  package = "plateforce",
  properties = list(
    values = S7::class_list,
    weighing_start_index = S7::class_integer,
    weighing_end_index = S7::class_integer,
    onset_index = S7::class_any,
    takeoff_index = S7::class_any,
    touchdown_index = S7::class_any,
    levels = S7::class_list,
    bound_methods = S7::class_list,
    bound_globals = S7::class_list,
    signals = S7::class_list,
    warnings = S7::class_character,
    refusals = S7::class_list,
    registry_version = S7::class_character,
    registry_declared_version = S7::class_character,
    registry_digest = S7::class_character
  )
)

#' @export
`print.plateforce::countermovement_jump` <- function(x, ...) {
  # A signal is said once, under the first value it qualifies.
  said_under <- vapply(x@signals, function(signal) signal@qualifies[1], character(1))
  for (name in names(x@values)) {
    value <- x@values[[name]]
    cat(sprintf(
      "%-40s %s %s\n",
      name,
      format(value@value, digits = 15),
      value@unit_symbol
    ))
    for (signal in x@signals[said_under == name]) {
      print(signal)
    }
  }
  for (warning_text in x@warnings) {
    cat(warning_text, "\n", sep = "")
  }
  invisible(x)
}

#' One quantity out of an analysis
#'
#' @param x A `countermovement_jump`.
#' @param quantity The engine's name for the quantity, matched in full.
#' @return A `measured`, the number with the rule that produced it attached.
#' @export
pf_value <- function(x, quantity) {
  if (!quantity %in% names(x@values)) {
    refuse_here(
      "quantity_not_reported",
      paste0("this analysis reported ", paste(names(x@values), collapse = ", ")),
      parameter = quantity,
      available = names(x@values)
    )
  }
  x@values[[quantity]]
}

#' Analyse a countermovement jump
#'
#' Three landmark rules are named by their registry identifiers, and every parameter each
#' one read travels in the result beside the number it produced.
#'
#' @param trial A `trial`, as returned by [pf_trial()] or [pf_read_force_file()].
#' @param weighing Registry id of the rule that establishes system weight.
#' @param onset Registry id of the rule that places the start of the jump.
#' @param takeoff Registry id of the rule that places the instant of takeoff.
#' @param gravity_meters_per_second_squared Gravitational acceleration. A bound parameter
#'   rather than a constant, because published tools disagree on it, and it appears in the
#'   record beside the number it moved.
#' @param body_mass_kilograms The athlete's mass, which is not the weighed system mass:
#'   system weight includes any bar and bodyweight does not. Absent, no mass is on the
#'   record, and a mass that is not a finite number above zero is refused by name.
#' @param weighing_parameters,onset_parameters,takeoff_parameters Named numeric lists, as
#'   the registry names each rule's parameters.
#' @param weighing_options,onset_options,takeoff_options Named character lists, for the
#'   settings that are a choice between named alternatives rather than a number.
#' @param weighing_start_index,onset_index,takeoff_index,touchdown_index Indices placed by
#'   hand. An override is a fact about what produced the number, so it travels in the
#'   record rather than replacing it.
#' @param derived Rules for constructs computed from the landmarks, named by the construct
#'   id the registry declares: `list(analysis_window = "window_end.takeoff.detected",
#'   peak_force = "force.peak.gross")`. A rule that takes values is written
#'   `list(peak_force = list(method_id = "force.peak.estimator",
#'   parameters = list(averaging_window_seconds = 0.1)))`. A rule that reads what another
#'   one placed declines by name when that construct was not named.
#' @param conditioning Rules and values for the phase that conditions the signal before the
#'   landmarks are placed, named by the construct id the registry declares:
#'   `list(conditioned_force_signal = "filter.none")`. Values are written
#'   `list(conditioned_force_signal = list(options = list(passband_edge = "none")))`, with the
#'   rule left out where the phase's own is wanted. The phase runs whether or not this is
#'   passed, so what it buys is the record naming the caller rather than the software.
#' @param preset Name of a published pipeline, which binds the rules and the values its
#'   source states and leaves every construct that source is silent about to the caller.
#'   Every value it supplies is recorded as cited, naming the pipeline, so a result reached
#'   this way is a different record from one reached by typing the same numbers.
#' @param registry Directory holding the registry, as in [pf_registry()].
#' @param registry_version The revision of the registry data to cite in the result. Absent,
#'   the result names no pinned revision and reports the one the registry declares for
#'   itself. The two are recorded separately: a revision the caller cited and one the data
#'   claimed about itself are different facts.
#' @return A `countermovement_jump`. Read one quantity out of it with [pf_value()].
#' @export
#' @examples
#' standing <- pf_trial(rep(700, 1200), sample_rate_hz = 1200)
#' result <- analyse_countermovement_jump(
#'   standing,
#'   weighing = "bwepoch.fixed_window",
#'   onset = "onset.threshold.noise_relative",
#'   takeoff = "takeoff.threshold.absolute_force"
#' )
#' pf_value(result, "system_weight_newtons")@value
#' result@warnings
analyse_countermovement_jump <- function(trial,
                                        weighing = NULL,
                                        onset = NULL,
                                        takeoff = NULL,
                                        preset = NULL,
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
                                        registry = NULL,
                                        registry_version = NULL) {
  request <- analysis_request_of(
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
    registry = registry, registry_version = registry_version
  )
  # The pipeline is laid on by the engine rather than here.
  reply <- if (is.null(preset)) {
    rust_analyse_json(trial@handle, registry_root(registry), request)
  } else {
    rust_analyse_under_preset_json(trial@handle, registry_root(registry), preset, request)
  }
  jump_from_response(unwrap(decode(reply)))
}

# The one place a request is written.
analysis_request_of <- function(weighing, onset, takeoff,
                                derived = NULL,
                                conditioning = NULL,
                                gravity_meters_per_second_squared = NULL,
                                body_mass_kilograms = NULL,
                                weighing_parameters = NULL, onset_parameters = NULL,
                                takeoff_parameters = NULL,
                                weighing_options = NULL, onset_options = NULL,
                                takeoff_options = NULL,
                                weighing_start_index = NULL, onset_index = NULL,
                                takeoff_index = NULL, touchdown_index = NULL,
                                registry = NULL, registry_version = NULL) {
  do.call(request_of, c(
    analysis_fields_of(
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
    registry_identity_of(registry, registry_version)
  ))
}

# What every record says about the registry behind it: the caller's pin, the registry's own
# claim, and the measured digest. Beside the analysis rather than inside it, because a sweep
# carries the same three against a request nested one level down.
registry_identity_of <- function(registry, registry_version) {
  drop_empty(list(
    registry_digest = registry_digest(registry),
    registry_version = registry_version,
    registry_declared_version = registry_declared_version(registry)
  ))
}

# The fields of one analysis request, as a list rather than as written JSON.
#
# A sweep varies an analysis request and carries it under `base`, so the two need the fields
# before they are written. Assembled a second time inside `pf_spread`, the sweep took five of
# the seventeen arguments the analysis takes and a caller could sweep around no derived
# construct, no conditioning rule, no placed landmark and no name a rule reads.
analysis_fields_of <- function(weighing, onset, takeoff,
                               derived = NULL,
                               conditioning = NULL,
                               gravity_meters_per_second_squared = NULL,
                               body_mass_kilograms = NULL,
                               weighing_parameters = NULL, onset_parameters = NULL,
                               takeoff_parameters = NULL,
                               weighing_options = NULL, onset_options = NULL,
                               takeoff_options = NULL,
                               weighing_start_index = NULL, onset_index = NULL,
                               takeoff_index = NULL, touchdown_index = NULL,
                               registry = NULL) {
  drop_empty(list(
    weighing = drop_empty(list(
      method_id = weighing,
      start_index = as_index(weighing_start_index),
      parameters = weighing_parameters,
      options = weighing_options
    )),
    onset = drop_empty(list(
      method_id = onset,
      parameters = onset_parameters,
      options = onset_options,
      manual_index = as_index(onset_index)
    )),
    takeoff = drop_empty(list(
      method_id = takeoff,
      parameters = takeoff_parameters,
      options = takeoff_options,
      manual_index = as_index(takeoff_index)
    )),
    derived = derived_choices(derived),
    conditioning = conditioning_choices(conditioning),
    touchdown_index = as_index(touchdown_index),
    gravity_meters_per_second_squared = gravity_meters_per_second_squared,
    gravity_source = gravity_claim(gravity_meters_per_second_squared),
    body_mass_kilograms = body_mass_kilograms,
    registry_backed_ids = registry_backed_ids(registry)
  ))
}

# A gravity the caller stated travels with the claim that they stated it. Without one the
# engine supplies the constant and records that nobody was asked.
gravity_claim <- function(gravity_meters_per_second_squared) {
  if (is.null(gravity_meters_per_second_squared)) NULL else "stated"
}

# A rule for something computed from the landmarks, keyed by the construct id the registry
# declares. The short form is the same request as the long one with nothing stated.
derived_choices <- function(derived) {
  if (is.null(derived) || !length(derived)) {
    return(NULL)
  }
  if (is.null(names(derived)) || any(!nzchar(names(derived)))) {
    refuse_here(
      "required_parameter_unstated",
      "each rule computed from the landmarks is named by its construct, as list(peak_force = \"force.peak.gross\")",
      parameter = "derived"
    )
  }
  lapply(derived, function(choice) {
    if (is.character(choice)) {
      return(list(method_id = choice))
    }
    drop_empty(list(
      method_id = choice[["method_id"]],
      parameters = choice[["parameters"]],
      options = choice[["options"]]
    ))
  })
}

# A rule and the values it reads for the phase that conditions the signal, keyed by the
# construct id the registry declares. The short form names a rule and states nothing.
#
# A choice may carry values and no rule, which is the caller stating what the phase's own rule
# reads without naming it. The id is left out of the request then, and the engine reads that as
# a construct nobody named a rule for: it runs the rule it declares and records it, which is
# what an absent construct already does.
conditioning_choices <- function(conditioning) {
  if (is.null(conditioning) || !length(conditioning)) {
    return(NULL)
  }
  if (is.null(names(conditioning)) || any(!nzchar(names(conditioning)))) {
    refuse_here(
      "required_parameter_unstated",
      "each rule that conditions the signal is named by its construct, as list(conditioned_force_signal = \"filter.none\")",
      parameter = "conditioning"
    )
  }
  lapply(conditioning, function(choice) {
    if (is.character(choice)) {
      return(list(method_id = choice))
    }
    drop_empty(list(
      method_id = choice[["method_id"]],
      parameters = choice[["parameters"]],
      options = choice[["options"]]
    ))
  })
}

drop_empty <- function(fields) fields[!vapply(fields, is.null, logical(1))]

# R indexes from one and the engine indexes from zero.
as_index <- function(index) {
  if (is.null(index)) {
    return(NULL)
  }
  as.integer(index) - 1L
}

jump_from_response <- function(response) {
  stamp <- registry_stamp_of(response)
  digest <- stamp$digest

  bound <- response[["bound_methods"]]
  by_method <- stats::setNames(bound, vapply(bound, function(b) b[["method_id"]], character(1)))

  accounts <- response[["descriptions"]]
  # One chain per quantity, in the order and the length of `metrics`, so a response naming one
  # key twice carries both records rather than a map keeping whichever arrived last.
  chains <- response[["provenance"]]

  values <- list()
  metrics <- response[["metrics"]]
  for (index in seq_along(metrics)) {
    metric <- metrics[[index]]
    account <- accounts[[metric[["key"]]]]
    values[[metric[["key"]]]] <- measured(
      value = if (is.null(metric[["value"]])) NA_real_ else as.double(metric[["value"]]),
      unit = as.character(metric[["unit"]]),
      unit_symbol = as.character(metric[["unit_symbol"]]),
      quantity = as.character(metric[["key"]]),
      provenance = provenance_from_record(chains[[index]]),
      account = if (is.null(account)) character(0) else as.character(account)
    )
  }

  countermovement_jump(
    values = values,
    weighing_start_index = as.integer(response[["weighing_start_index"]]) + 1L,
    weighing_end_index = as.integer(response[["weighing_end_index"]]) + 1L,
    onset_index = one_based(response[["onset_index"]]),
    takeoff_index = one_based(response[["takeoff_index"]]),
    touchdown_index = one_based(response[["touchdown_index"]]),
    levels = response[["levels"]],
    bound_methods = by_method,
    # What the whole analysis was bound to, keyed by the name the record reports each value
    # by. No rule's row can carry these: they belong to the request rather than to an entry.
    bound_globals = by_name(response[["bound_globals"]]),
    signals = lapply(response[["signals"]], signal_from_list),
    warnings = as.character(unlist(response[["warnings"]])),
    refusals = lapply(response[["refusals"]], refusal_condition),
    registry_version = stamp$version,
    registry_declared_version = stamp$declared_version,
    registry_digest = digest
  )
}

one_based <- function(index) if (is.null(index)) NULL else as.integer(index) + 1L

# A value the request bound for the whole analysis, reachable by its own name rather than by
# position, because a caller asking what mass a result ran under knows the name and not the
# order. A request that bound none gives an empty list.
by_name <- function(bound) {
  if (is.null(bound) || !length(bound)) {
    return(list())
  }
  stats::setNames(bound, vapply(bound, function(one) one[["name"]], character(1)))
}
