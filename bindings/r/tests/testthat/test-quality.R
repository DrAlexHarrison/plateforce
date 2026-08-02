# What the software knows about a number it hands back, on the surface whose numbers get
# pasted into a paper. Every expected value is read from tests/testthat/fixtures/, which
# tools/generate-fixtures.R writes by running the engine.

DISAGREEMENT <- "disagreement.txt"

# The rule whose two routes to a height disagree on this trial, and one whose routes agree.
# Without the second, a passing assertion could be about a signal that always fires.
DISAGREEING_RULE <- "onset.threshold.last_within_band"
AGREEING_RULE <- "onset.threshold.noise_relative"

analysed_with <- function(rule) {
  trace <- repository_trace()
  testthat::skip_if(is.null(trace), "the trial these fixtures were derived from is not here")
  trial <- pf_read_force_file(trace, sample_rate_hz = 1200, delimiter = "\t",
                              force_column = 0, sentinel_convention = "none")
  analyse_countermovement_jump(
    trial,
    weighing = "bwepoch.fixed_window",
    onset = rule,
    takeoff = "takeoff.threshold.absolute_force"
  )
}

disagreement_percent <- function(result) {
  takeoff <- pf_value(result, "jump_height_from_takeoff_meters")@value
  flight <- pf_value(result, "jump_height_from_flight_time_meters")@value
  100 * abs(takeoff - flight) / flight
}

test_that("a rule whose two routes to one height disagree raises one signal naming both", {
  result <- analysed_with(DISAGREEING_RULE)

  expect_length(result@signals, as.integer(fixture_field(DISAGREEMENT, "signal_count")))
  signal <- result@signals[[1]]
  expect_identical(signal@status, fixture_field(DISAGREEMENT, "status"))
  expect_identical(signal@unit, fixture_field(DISAGREEMENT, "unit"))
  expect_identical(signal@remedy_construct, fixture_field(DISAGREEMENT, "remedy_construct"))
  expect_identical(
    signal@qualifies,
    strsplit(fixture_field(DISAGREEMENT, "qualifies"), " ", fixed = TRUE)[[1]]
  )
  expect_equal(signal@value, as.double(fixture_field(DISAGREEMENT, "value")))
  expect_equal(signal@threshold, as.double(fixture_field(DISAGREEMENT, "threshold")))
  expect_true(nzchar(signal@remedy))
})

test_that("the two heights the signal is about are the two heights this rule produces", {
  result <- analysed_with(DISAGREEING_RULE)

  expect_equal(
    pf_value(result, "jump_height_from_takeoff_meters")@value,
    as.double(fixture_field(DISAGREEMENT, "jump_height_from_takeoff_meters"))
  )
  expect_equal(
    pf_value(result, "jump_height_from_flight_time_meters")@value,
    as.double(fixture_field(DISAGREEMENT, "jump_height_from_flight_time_meters"))
  )
  # The relation the signal claims, taken from the two numbers rather than from the record
  # of them, so a baseline captured under a wrong value cannot satisfy this one.
  expect_gt(disagreement_percent(result), result@signals[[1]]@threshold)
})

test_that("a rule whose two routes agree says nothing", {
  result <- analysed_with(AGREEING_RULE)

  expect_length(result@signals, as.integer(fixture_field(DISAGREEMENT, "agreeing_signal_count")))
  expect_lt(disagreement_percent(result), as.double(fixture_field(DISAGREEMENT, "threshold")))
})

test_that("the signal is said once, beside the value it is about", {
  result <- analysed_with(DISAGREEING_RULE)
  printed <- capture.output(print(result))

  qualified <- grep("^jump_height_from_takeoff_meters", printed)[1]
  following <- grep("^jump_height_from_flight_time_meters", printed)[1]
  said <- grep(result@signals[[1]]@remedy, printed, fixed = TRUE)

  expect_length(said, 1L)
  expect_true(qualified < said[1] && said[1] < following)
})

test_that("a signal is not a refusal", {
  result <- analysed_with(DISAGREEING_RULE)

  expect_false(is.na(pf_value(result, "jump_height_from_takeoff_meters")@value))
  expect_identical(result@warnings, character(0))
})
