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

test_that("a parameter the caller stated is recorded as stated, not as assumed", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    weighing_parameters = list(duration = 0.5),
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  bound <- pf_value(result, "system_weight_newtons")@provenance@depends_on[[1]]@parameters
  duration <- bound[bound[["name"]] == "duration", , drop = FALSE]

  expect_identical(duration[["source"]], "stated")
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
