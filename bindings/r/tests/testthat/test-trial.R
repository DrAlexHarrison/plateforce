test_that("a trial reports what it holds", {
  trial <- pf_trial(rep(700, 1200), sample_rate_hz = 1200)

  expect_identical(trial@sample_count, 1200L)
  expect_identical(trial@duration_seconds, 1)
  expect_identical(trial@sample_rate_hz, 1200)
})

test_that("a rate that was not declared is refused by naming the argument", {
  condition <- tryCatch(pf_trial(rep(700, 1200)), plateforce_refusal = identity)

  expect_s3_class(condition, "plateforce_required_parameter_unstated")
  expect_identical(condition[["parameter"]], "sample_rate_hz")
})

test_that("an integer trace is refused rather than widened", {
  condition <- tryCatch(
    pf_trial(rep(700L, 1200), sample_rate_hz = 1200),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_force_not_double")
  expect_match(conditionMessage(condition), "as.double()", fixed = TRUE)
})

test_that("a sentinel convention nobody applies is refused with the ones that are", {
  condition <- tryCatch(
    pf_trial(rep(700, 12), sample_rate_hz = 12, sentinel_convention = "nine_thousand"),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_sentinel_convention_unknown")
  expect_setequal(condition[["available"]], list("none", "zero", "negative_one"))
})

test_that("a sentinel is counted rather than folded into the trace", {
  force <- rep(700, 100)
  force[c(10, 20, 30)] <- 0
  trial <- pf_trial(force, sample_rate_hz = 100, sentinel_convention = "zero")

  expect_identical(trial@read_report[["samples_matching_the_convention"]], 3L)
  expect_identical(trial@read_report[["samples_carrying_no_number"]], 0L)
  expect_identical(trial@sample_count, 100L)
})

test_that("a gap in the recording is counted apart from the convention's own matches", {
  force <- rep(700, 100)
  force[c(10, 20, 30)] <- 0
  force[c(50, 51)] <- NaN
  trial <- pf_trial(force, sample_rate_hz = 100, sentinel_convention = "zero")

  # Five samples are reported and one number cannot say which is which. The three zeros are
  # the caller's declaration meeting real data; the two gaps are the recording itself, and
  # they would still be there under any convention.
  expect_identical(trial@read_report[["samples_matching_the_convention"]], 3L)
  expect_identical(trial@read_report[["samples_carrying_no_number"]], 2L)

  # The control that says the split is real rather than a relabelling: declaring nothing
  # empties one count and leaves the other exactly where it was.
  undeclared <- pf_trial(force, sample_rate_hz = 100, sentinel_convention = "none")
  expect_identical(undeclared@read_report[["samples_matching_the_convention"]], 0L)
  expect_identical(undeclared@read_report[["samples_carrying_no_number"]], 2L)
})

test_that("the trace comes back as the doubles that went in", {
  force <- as.double(seq_len(50) + 600)
  trial <- pf_trial(force, sample_rate_hz = 50)

  expect_identical(pf_force(trial), force)
})
