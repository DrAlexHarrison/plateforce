# R's answer to the one committed request.
#
# The request is read through the engine's own decoder rather than through a JSON package,
# so this arm needs nothing the package does not already ship, and the document comes back
# as the engine wrote it inside R's process rather than after R has reshaped it.

asked <- plateforce:::decode(paste(
  readLines(Sys.getenv("PLATEFORCE_PARITY_REQUEST"), warn = FALSE),
  collapse = "\n"
))

trial <- plateforce::pf_read_force_file(
  asked$trial,
  sample_rate_hz = asked$sample_rate_hz,
  delimiter = asked$delimiter,
  force_column = asked$force_column,
  sentinel_convention = asked$sentinel_convention
)

slot_of <- function(named) {
  list(method_id = named$method_id, parameters = named$parameters)
}

cat(plateforce:::rust_analyse_json(trial@handle, plateforce:::request_of(
  weighing = slot_of(asked$weighing),
  onset = slot_of(asked$onset),
  takeoff = slot_of(asked$takeoff),
  registry_digest = plateforce:::registry_digest(NULL)
)))
