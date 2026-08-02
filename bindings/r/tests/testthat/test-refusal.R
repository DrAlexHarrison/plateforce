declined <- function() {
  tryCatch(
    analyse_countermovement_jump(
      quiet_trial(),
      weighing = "bwepoch.fixed_window",
      onset = "onset.not.a.rule",
      takeoff = "takeoff.threshold.absolute_force"
    ),
    error = identity
  )
}

test_that("a refusal arrives classed by its code, as a refusal, and as an error", {
  condition <- declined()

  expect_identical(
    class(condition),
    c("plateforce_analysis_declined", "plateforce_refusal", "plateforce_error",
      "error", "condition")
  )
  expect_identical(condition[["code"]], "analysis_declined")
})

test_that("both the specific and the general handler catch one refusal", {
  condition <- declined()

  expect_identical(
    tryCatch(stop(condition), plateforce_analysis_declined = function(c) "specific"),
    "specific"
  )
  expect_identical(
    tryCatch(stop(condition), plateforce_refusal = function(c) "general"),
    "general"
  )
})

test_that("a field is read by its whole name", {
  condition <- declined()

  expect_error(condition$method)
  expect_error(condition$param)
  expect_identical(condition[["code"]], "analysis_declined")
})

test_that("every field the idiom names is present on a refusal", {
  condition <- declined()
  carried <- names(unclass(condition))

  for (field in c("code", "method_id", "slot", "parameter", "value", "detail", "available")) {
    expect_true(field %in% carried, info = field)
  }
})
