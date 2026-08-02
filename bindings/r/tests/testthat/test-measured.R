test_that("a shortened property name is an error rather than a neighbour's value", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  weight <- pf_value(result, "system_weight_newtons")

  expect_error(weight@val)
  expect_error(weight@jump_height)
  expect_identical(weight@value, 700)
  expect_identical(weight@unit, "newtons")
})

test_that("printing a measured shows the number, the unit and the quantity", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  printed <- capture.output(print(pf_value(result, "system_weight_newtons")))

  expect_match(printed[1], "700")
  expect_match(printed[1], "N")
  expect_match(printed[1], "system_weight_newtons")
})

test_that("a quantity the analysis did not report is refused by name", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  condition <- tryCatch(pf_value(result, "jump_height"), plateforce_refusal = identity)

  expect_s3_class(condition, "plateforce_quantity_not_reported")
  expect_identical(condition[["parameter"]], "jump_height")
})
