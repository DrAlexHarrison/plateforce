# An R session states the athlete's mass and reads it back as its own claim.
#
# The athlete's mass is not the weighed system mass: system weight includes any bar and
# bodyweight does not. A surface with no way to say which leaves a caller substituting the
# number beside it, and nothing in the record can tell the two apart afterwards.

STATED_MASS_KILOGRAMS <- 61.5
STANDARD_GRAVITY <- 9.80665

analysed <- function(...) {
  analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force",
    ...
  )
}

test_that("a mass the caller states is on the record under the caller's own claim", {
  bound <- analysed(body_mass_kilograms = STATED_MASS_KILOGRAMS)@bound_globals

  expect_identical(bound[["body_mass_kilograms"]][["value"]], STATED_MASS_KILOGRAMS)
  expect_identical(bound[["body_mass_kilograms"]][["source"]], "stated")
  # The unit travels with the number, because a mass read back without one is a number the
  # caller has to assume the units of.
  expect_identical(bound[["body_mass_kilograms"]][["unit"]], "kilograms")
  expect_identical(bound[["body_mass_kilograms"]][["unit_symbol"]], "kg")
})

test_that("a run that states no mass carries no row for one and still names its gravity", {
  bound <- analysed()@bound_globals

  expect_false("body_mass_kilograms" %in% names(bound))
  # More than one value on purpose: a record holding no row at all would satisfy the line
  # above while proving nothing about the shape that holds a row.
  expect_identical(bound[["gravity_meters_per_second_squared"]][["source"]], "assumed")
  expect_equal(
    bound[["gravity_meters_per_second_squared"]][["value"]],
    STANDARD_GRAVITY
  )
})

test_that("both bound values are on the record when the caller states both", {
  bound <- analysed(
    body_mass_kilograms = STATED_MASS_KILOGRAMS,
    gravity_meters_per_second_squared = 9.81
  )@bound_globals

  claims <- vapply(bound, function(one) one[["source"]], character(1))
  expect_identical(
    claims[order(names(claims))],
    c(body_mass_kilograms = "stated", gravity_meters_per_second_squared = "stated")
  )
})

test_that("a mass that is not a positive finite number is refused under its own name", {
  for (kilograms in list(0, -61.5, NaN, Inf, -Inf, NA_real_)) {
    raised <- tryCatch(
      analysed(body_mass_kilograms = kilograms),
      plateforce_error = function(condition) condition
    )
    expect_s3_class(raised, "plateforce_refusal")
    expect_true(
      grepl("body_mass_kilograms", conditionMessage(raised), fixed = TRUE),
      label = paste("the refusal for", format(kilograms), "names the parameter")
    )
  }
})

test_that("a non-finite number in any request field is refused rather than sent as null", {
  # `NaN` and `NA_real_` used to leave R as JSON null, which the engine reads as a value
  # nobody stated, so a caller who typed one got a run recorded as having stated nothing.
  # The infinities left as bare `Inf`, which is not JSON and came back naming a column of
  # the document. A rule's own parameter takes the same route, so it is asked here too.
  raised <- tryCatch(
    analysed(onset_parameters = list(k = NaN)),
    plateforce_error = function(condition) condition
  )
  expect_s3_class(raised, "plateforce_refusal")
  expect_true(grepl("k", conditionMessage(raised), fixed = TRUE))
})

test_that("the sweep sends the mass an analysis of the same trial sends", {
  # A sweep's unvaried combination has to be the request the caller's own analysis sends, or
  # the spread is around a different result than the one they read.
  swept <- pf_spread(
    quiet_trial(),
    quantity = "system_weight_newtons",
    slot = "weighing",
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force",
    body_mass_kilograms = STATED_MASS_KILOGRAMS
  )
  expect_true(swept[["succeeded"]] > 0)

  raised <- tryCatch(
    pf_spread(
      quiet_trial(),
      quantity = "system_weight_newtons",
      slot = "weighing",
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force",
      body_mass_kilograms = 0
    ),
    plateforce_error = function(condition) condition
  )
  expect_s3_class(raised, "plateforce_refusal")
})
