#' @include measured.R provenance.R trial.R
NULL

#' One countermovement jump, analysed
#'
#' `@values` is a named list of [measured] objects, one per quantity the engine reported,
#' keyed by the engine's own name for it. Read one with `x@values[["name"]]`, which
#' matches the whole name, or with [pf_value()].
#'
#' The names come from the engine rather than from a list written here, so a quantity that
#' arrives or is renamed upstream arrives or is renamed here without an edit.
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
    signals = S7::class_list,
    warnings = S7::class_character,
    registry_digest = S7::class_character
  )
)

#' @export
`print.plateforce::countermovement_jump` <- function(x, ...) {
  # A signal is said once, under the first value it qualifies, because a reader scanning
  # the values does not reach a block at the end.
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
#' @return A [measured].
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
#' @param trial A [trial].
#' @param weighing Registry id of the rule that establishes system weight.
#' @param onset Registry id of the rule that places the start of the jump.
#' @param takeoff Registry id of the rule that places the instant of takeoff.
#' @param gravity_meters_per_second_squared Gravitational acceleration. A bound parameter
#'   rather than a constant, because published tools disagree on it, and it appears in the
#'   record beside the number it moved.
#' @param weighing_parameters,onset_parameters,takeoff_parameters Named numeric lists, as
#'   the registry names each rule's parameters.
#' @param weighing_options,onset_options,takeoff_options Named character lists, for the
#'   settings that are a choice between named alternatives rather than a number.
#' @param weighing_start_index,onset_index,takeoff_index,touchdown_index Indices placed by
#'   hand. An override is a fact about what produced the number, so it travels in the
#'   record rather than replacing it.
#' @param registry Directory holding the registry, as in [pf_registry()].
#' @return A [countermovement_jump].
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
                                        weighing,
                                        onset,
                                        takeoff,
                                        gravity_meters_per_second_squared = NULL,
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
                                        registry = NULL) {
  request <- request_of(
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
    touchdown_index = as_index(touchdown_index),
    gravity_meters_per_second_squared = gravity_meters_per_second_squared,
    registry_digest = registry_digest(registry)
  )
  response <- unwrap(decode(rust_analyse_json(trial@handle, request)))
  jump_from_response(response)
}

drop_empty <- function(fields) fields[!vapply(fields, is.null, logical(1))]

# R indexes from one and the engine indexes from zero. The conversion happens here, once,
# rather than at each call site.
as_index <- function(index) {
  if (is.null(index)) {
    return(NULL)
  }
  as.integer(index) - 1L
}

jump_from_response <- function(response) {
  digest <- response[["registry_digest"]]
  if (is.null(digest)) digest <- character(0)
  complete <- isTRUE(response[["acquisition_complete"]])

  bound <- response[["bound_methods"]]
  by_method <- stats::setNames(bound, vapply(bound, function(b) b[["method_id"]], character(1)))

  # One record per rule, built once. Eleven quantities naming the same eight rules would
  # otherwise build the same record eighty-eight times.
  records <- lapply(by_method, provenance_from_bound_method, digest, complete)

  accounts <- response[["descriptions"]]

  values <- list()
  for (metric in response[["metrics"]]) {
    chain <- records[as.character(unlist(metric[["contributing_method_ids"]]))]
    names(chain) <- NULL
    computed_by <- metric[["computed_by"]]
    own <- provenance(
      method_id = if (is.null(computed_by)) character(0) else as.character(computed_by),
      parameters = EMPTY_BINDING(),
      choices = EMPTY_BINDING(),
      registry_version = character(0),
      registry_digest = digest,
      acquisition_complete = complete,
      depends_on = chain
    )
    account <- accounts[[metric[["key"]]]]
    values[[metric[["key"]]]] <- measured(
      value = if (is.null(metric[["value"]])) NA_real_ else as.double(metric[["value"]]),
      unit = as.character(metric[["unit"]]),
      unit_symbol = as.character(metric[["unit_symbol"]]),
      quantity = as.character(metric[["key"]]),
      provenance = own,
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
    signals = lapply(response[["signals"]], signal_from_list),
    warnings = as.character(unlist(response[["warnings"]])),
    registry_digest = digest
  )
}

one_based <- function(index) if (is.null(index)) NULL else as.integer(index) + 1L
