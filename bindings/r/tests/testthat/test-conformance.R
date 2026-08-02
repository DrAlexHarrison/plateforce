# The trial these numbers were derived from lives in the repository rather than in the
# package, so this runs where the trial is. Every number compared here was written by
# tools/generate-fixtures.R running the engine, and none was typed by a person.

analysed <- function() {
  trace <- repository_trace()
  testthat::skip_if(is.null(trace), "the trial these fixtures were derived from is not here")
  trial <- pf_read_force_file(trace, sample_rate_hz = 1200, delimiter = "\t",
                              force_column = 0, sentinel_convention = "none")
  list(
    trial = trial,
    result = analyse_countermovement_jump(
      trial,
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    )
  )
}

test_that("the reader reports what the record says it read", {
  run <- analysed()

  expect_identical(as.character(run$trial@sample_count), fixture_field("trial.txt", "sample_count"))
  expect_identical(as.character(run$trial@read_report[["rows_read"]]),
                   fixture_field("trial.txt", "rows_read"))
  expect_identical(as.character(run$trial@read_report[["samples_treated_as_missing"]]),
                   fixture_field("trial.txt", "samples_treated_as_missing"))
})

test_that("every quantity equals the number the engine produced for it", {
  run <- analysed()
  recorded <- fixture_lines("values.txt")

  live <- vapply(names(run$result@values), function(name) {
    value <- run$result@values[[name]]
    paste(name, format(value@value, digits = 17), value@unit, value@unit_symbol)
  }, character(1))

  expect_identical(unname(live), recorded)
})

test_that("the landmarks equal the ones the engine placed", {
  run <- analysed()
  result <- run$result

  expect_identical(as.character(result@onset_index), fixture_field("landmarks.txt", "onset_index"))
  expect_identical(as.character(result@takeoff_index),
                   fixture_field("landmarks.txt", "takeoff_index"))
})

test_that("the chain behind a jump height is the chain the engine bound", {
  run <- analysed()
  chain <- run$result@values[["jump_height_from_takeoff_meters"]]@provenance@depends_on
  live <- vapply(chain, function(step) {
    bound <- step@parameters
    paste(step@method_id,
          paste(paste0(bound[["name"]], "=", bound[["value"]], "(", bound[["source"]], ")"),
                collapse = " "))
  }, character(1))

  expect_identical(live, fixture_lines("chain.txt"))
})

test_that("two published routes to one jump height disagree, and both are named", {
  run <- analysed()
  from_takeoff <- pf_value(run$result, "jump_height_from_takeoff_meters")
  from_flight <- pf_value(run$result, "jump_height_from_flight_time_meters")

  expect_false(identical(from_takeoff@value, from_flight@value))
  expect_true(length(from_takeoff@provenance@depends_on) > 0)
  expect_true(length(from_flight@provenance@depends_on) > 0)
})
