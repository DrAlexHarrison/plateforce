test_that("a trace with no jump in it reports no landmark and says which rule declined", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_null(result@onset_index)
  expect_null(result@takeoff_index)
  expect_true(is.na(pf_value(result, "jump_height_from_takeoff_meters")@value))
  expect_true(length(result@warnings) >= 2)
  expect_true(any(grepl("onset.threshold.noise_relative", result@warnings, fixed = TRUE)))
  expect_true(any(grepl("takeoff.threshold.absolute_force", result@warnings, fixed = TRUE)))
})

test_that("a value a rule did compute carries the rule and what it was bound to", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  weight <- pf_value(result, "system_weight_newtons")
  chain <- weight@provenance@depends_on

  expect_true(length(chain) >= 1)
  expect_identical(chain[[1]]@method_id, "bwepoch.fixed_window")
  expect_true(nrow(chain[[1]]@parameters) > 0)
  expect_true(all(chain[[1]]@parameters[["source"]] %in%
    c("stated", "assumed", "measured", "provisional")))
})

weighed <- function(...) {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force",
    ...
  )
  pf_value(result, "system_weight_newtons")@provenance@depends_on[[1]]@parameters
}

source_of <- function(bound, name) bound[bound[["name"]] == name, "source"]

test_that("stating a value is what makes the record say the caller stated it", {
  # Two runs alike in everything except whether the caller stated the duration. Asserting
  # only that a stated value reads stated passes just as well when every value does.
  stated <- weighed(weighing_parameters = list(duration = 0.5))
  unstated <- weighed()

  expect_identical(source_of(stated, "duration"), "stated")
  expect_identical(source_of(unstated, "duration"), "assumed")
  expect_identical(source_of(stated, "centre"), source_of(unstated, "centre"))
  expect_false(source_of(unstated, "centre") == "stated")
})

test_that("a value the rule measured off this trace is not recorded as one it assumed", {
  bound <- weighed()

  # The weighing epoch's start is read from the trace rather than defaulted, and a record
  # that called it assumed would name a source no author chose.
  expect_identical(source_of(bound, "start_seconds"), "measured")
})

test_that("a dataset with no acquisition block fingerprints as incomplete", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_false(pf_value(result, "system_weight_newtons")@provenance@acquisition_complete)
})

test_that("the registry the numbers were bound to is named in the record", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_identical(result@registry_digest, pf_registry()@digest)
})
