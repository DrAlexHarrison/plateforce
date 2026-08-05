# R's answer to the one committed request.
#
# The request is read through the engine's own decoder rather than through a JSON package,
# so this arm needs nothing the package does not already ship, and the document comes back
# as the engine wrote it inside R's process rather than after R has reshaped it.

asked <- plateforce:::decode(paste(
  readLines(Sys.getenv("PLATEFORCE_PARITY_REQUEST"), warn = FALSE),
  collapse = "\n"
))

# What the plate was, and which saved plate the answers were typed into.
#
# Through the block's own builder, so a name the block does not hold stops this arm rather
# than travelling as an answer nobody asked for.
block_of <- function(members) {
  if (is.null(members)) {
    return(NULL)
  }
  if (!is.null(members$plate_natural_frequency_hz)) {
    members$plate_natural_frequency_hz <- as.numeric(members$plate_natural_frequency_hz)
  }
  do.call(plateforce::pf_acquisition, members)
}

# The plate travels as its name and its members rather than as a path, because the request is
# answered on four machines and none of them saved it. `pf_plate` takes the revision from the
# members, so the attribution this arm produces is the one the terminal produces off its own
# store. Naming a plate this machine holds is the same call without `acquisition`.
acquisition <- NULL
plate <- NULL
if (!is.null(asked$capture)) {
  acquisition <- block_of(asked$capture$acquisition)
  if (!is.null(asked$capture$plate)) {
    plate <- plateforce::pf_plate(
      asked$capture$plate$name,
      block_of(asked$capture$plate$members)
    )
  }
}

trial <- plateforce::pf_read_force_file(
  asked$trial,
  sample_rate_hz = asked$sample_rate_hz,
  delimiter = asked$delimiter,
  force_column = asked$force_column,
  sentinel_convention = asked$sentinel_convention,
  acquisition = acquisition,
  plate = plate
)

# A request carrying a `sweep` block asks how far the number moves across the slots it names,
# and one without it asks what the analysis reports. The terminal's arm, the notebook's and
# the browser's make the same test in the same words.
#
# The builder a user's own call goes through, rather than one assembled here. An arm that
# wrote its own request would send a document nobody sends, and the comparison would be
# measuring that document instead of the product's.
bound <- list(
  derived = asked$derived,
  weighing = asked$weighing$method_id,
  onset = asked$onset$method_id,
  takeoff = asked$takeoff$method_id,
  weighing_parameters = asked$weighing$parameters,
  onset_parameters = asked$onset$parameters,
  takeoff_parameters = asked$takeoff$parameters
)

if (is.null(asked$sweep)) {
  cat(plateforce:::rust_analyse_json(
    trial@handle,
    plateforce:::registry_root(NULL),
    do.call(plateforce:::analysis_request_of, bound)
  ))
} else {
  cat(plateforce:::rust_spread_json(trial@handle, plateforce:::registry_root(NULL), do.call(
    plateforce:::spread_request_of,
    c(
      list(
        quantity = asked$sweep$quantity_key,
        slot = unlist(asked$sweep$slots),
        maximum_combinations = asked$sweep$maximum_combinations
      ),
      bound
    )
  )))
}
