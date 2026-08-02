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

# The builder a user's own call goes through, rather than one assembled here. An arm that
# wrote its own request would send a document nobody sends, and the comparison would be
# measuring that document instead of the product's.
cat(plateforce:::rust_analyse_json(trial@handle, plateforce:::analysis_request_of(
  weighing = asked$weighing$method_id,
  onset = asked$onset$method_id,
  takeoff = asked$takeoff$method_id,
  weighing_parameters = asked$weighing$parameters,
  onset_parameters = asked$onset$parameters,
  takeoff_parameters = asked$takeoff$parameters
)))
