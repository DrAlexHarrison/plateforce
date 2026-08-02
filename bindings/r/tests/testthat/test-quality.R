# What the software knows about a number it hands back, on the surface whose numbers get
# pasted into a paper. Every expected value is read from tests/testthat/fixtures/, which
# tools/generate-fixtures.R writes by running the engine.

ROUTES <- "disagreement.txt"

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

for (position in c("first", "second")) {
  local({
    prefix <- position
    rule <- fixture_field(ROUTES, paste0(prefix, "_rule"))

    test_that(paste("the two routes to one height under", rule, "are the engine's"), {
      result <- analysed_with(rule)

      expect_equal(
        pf_value(result, "jump_height_from_takeoff_meters")@value,
        as.double(fixture_field(ROUTES, paste0(prefix, "_takeoff_meters")))
      )
      expect_equal(
        pf_value(result, "jump_height_from_flight_time_meters")@value,
        as.double(fixture_field(ROUTES, paste0(prefix, "_flight_meters")))
      )
      # Taken from the two numbers rather than from the record of them, so a baseline
      # captured under a wrong value cannot satisfy it.
      expect_equal(
        disagreement_percent(result),
        as.double(fixture_field(ROUTES, paste0(prefix, "_percent")))
      )
    })

    test_that(paste("what this rule raises is what its two numbers warrant, under", rule), {
      result <- analysed_with(rule)
      raised <- length(result@signals)

      expect_identical(
        raised, as.integer(fixture_field(ROUTES, paste0(prefix, "_signal_count")))
      )
      # The relation behind the count. A run raising nothing while the two routes sat far
      # apart would be a signal that did not fire, and the count alone cannot tell those
      # apart from a rule whose routes agree.
      if (raised == 0L) {
        expect_lt(disagreement_percent(result), 20)
      } else {
        expect_gt(disagreement_percent(result), result@signals[[1]]@threshold)
      }
    })
  })
}

test_that("a signal arrives with every field it carries", {
  # No shipped onset rule disagrees on this trial, so the conversion is held to a document
  # rather than left unexercised. A field the engine adds and this drops is a field an R
  # reader never sees.
  raised <- plateforce:::signal_from_list(list(
    label = "Jump height, two routes",
    value = 66.28,
    unit = "percent",
    threshold = 20,
    status = "disagrees",
    remedy = "name a different rule for the start of the jump",
    remedy_construct = "movement_onset",
    qualifies = list("jump_height_from_takeoff_meters", "jump_height_from_flight_time_meters")
  ))

  expect_identical(raised@status, "disagrees")
  expect_identical(raised@unit, "percent")
  expect_identical(raised@remedy_construct, "movement_onset")
  expect_length(raised@qualifies, 2L)
  expect_equal(raised@value, 66.28)
  expect_equal(raised@threshold, 20)

  printed <- capture.output(print(raised))
  expect_true(any(grepl(raised@remedy, printed, fixed = TRUE)))
  expect_true(any(grepl("percent", printed, fixed = TRUE)))
})

test_that("a comparison that could not be made carries no value rather than a zero", {
  incomparable <- plateforce:::signal_from_list(list(
    label = "Jump height, two routes",
    value = NULL,
    unit = "percent",
    threshold = 20,
    status = "incomparable",
    remedy = "name a rule that places takeoff",
    remedy_construct = "movement_onset",
    qualifies = list("jump_height_from_takeoff_meters")
  ))

  expect_true(is.na(incomparable@value))
  expect_identical(incomparable@status, "incomparable")
  expect_true(any(grepl("not comparable", capture.output(print(incomparable)), fixed = TRUE)))
})
