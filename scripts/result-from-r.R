# R's answer to the one committed request.
#
# The request is read through the engine's own decoder rather than through a JSON package,
# so this arm needs nothing the package does not already ship, and the document comes back
# as the engine wrote it inside R's process rather than after R has reshaped it.

asked <- plateforce:::decode(paste(
  readLines(Sys.getenv("PLATEFORCE_PARITY_REQUEST"), warn = FALSE),
  collapse = "\n"
))

# A request carrying a `sweep` block asks how far the number moves across several slots at
# once, and `pf_spread` takes one slot per call. Answering it with the analysis below would
# put an analysed document where a swept one is expected, so this says which question it
# cannot be asked instead. `SURFACES_NOT_ASKED` in scripts/result_parity.py names the work.
if (!is.null(asked$sweep)) {
  stop("pf_spread sweeps one slot per call and this request names ",
       length(asked$sweep$slots), call. = FALSE)
}

# What the plate was, as far as this surface can state it. An R session holds no store of
# saved plates, so it types the answers the request states and lays the ones stated on the
# capture over the ones the plate holds, which is what the two surfaces reading a saved plate
# do with them. What it cannot say is which saved plate they came off: the record this surface
# produces carries the members and no attribution, and `plate_profile` in
# scripts/result_parity.py is where that gap is recorded with the work that closes it.
acquisition <- NULL
if (!is.null(asked$capture)) {
  members <- asked$capture$plate$members
  for (member in names(asked$capture$acquisition)) {
    members[[member]] <- asked$capture$acquisition[[member]]
  }
  if (!is.null(members$plate_natural_frequency_hz)) {
    members$plate_natural_frequency_hz <- as.numeric(members$plate_natural_frequency_hz)
  }
  # Through the block's own builder, so a name the block does not hold stops this arm rather
  # than travelling as an answer nobody asked for.
  acquisition <- do.call(plateforce::pf_acquisition, members)
}

trial <- plateforce::pf_read_force_file(
  asked$trial,
  sample_rate_hz = asked$sample_rate_hz,
  delimiter = asked$delimiter,
  force_column = asked$force_column,
  sentinel_convention = asked$sentinel_convention,
  acquisition = acquisition
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
