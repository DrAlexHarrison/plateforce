test_that("dropping the provenance is refused on every route out of a measured", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  weight <- pf_value(result, "system_weight_newtons")

  for (route in list(
    function() as.numeric(weight),
    function() as.double(weight),
    function() weight + 1,
    function() 1 + weight
  )) {
    condition <- tryCatch(route(), error = identity)
    expect_s3_class(condition, "plateforce_refusal")
    expect_match(conditionMessage(condition), "@value is the number", fixed = TRUE)
  }
})

test_that("the number is reachable by naming it", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_type(pf_value(result, "system_weight_newtons")@value, "double")
})
