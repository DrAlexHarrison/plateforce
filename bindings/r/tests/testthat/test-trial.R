test_that("a trial reports what it holds", {
  trial <- pf_trial(rep(700, 1200), sample_rate_hz = 1200)

  expect_identical(trial@sample_count, 1200L)
  expect_identical(trial@duration_seconds, 1)
  expect_identical(trial@sample_rate_hz, 1200)
})

test_that("a rate that was not declared is refused by naming the argument", {
  condition <- tryCatch(pf_trial(rep(700, 1200)), plateforce_refusal = identity)

  expect_s3_class(condition, "plateforce_sample_rate_not_declared")
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

  expect_s3_class(condition, "plateforce_unknown_sentinel_convention")
  expect_setequal(condition[["available"]], list("none", "zero", "negative_one"))
})

test_that("a sentinel is counted rather than folded into the trace", {
  force <- rep(700, 100)
  force[c(10, 20, 30)] <- 0
  trial <- pf_trial(force, sample_rate_hz = 100, sentinel_convention = "zero")

  expect_identical(trial@read_report[["samples_treated_as_missing"]], 3L)
  expect_identical(trial@sample_count, 100L)
})

test_that("the trace comes back as the doubles that went in", {
  force <- as.double(seq_len(50) + 600)
  trial <- pf_trial(force, sample_rate_hz = 50)

  expect_identical(pf_force(trial), force)
})
