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

# An analysis that ends before any number computes used to be wrapped here under
# `analysis_declined`, a name this package's own manifest does not publish, so the class a
# caller would catch it by did not exist. It arrives under the code the rule declined on.
test_that("a refusal arrives classed by its code, as a refusal, and as an error", {
  condition <- declined()

  expect_identical(
    class(condition),
    c("plateforce_method_not_implemented", "plateforce_refusal", "plateforce_error",
      "error", "condition")
  )
  expect_identical(condition[["code"]], "method_not_implemented")

  manifest <- jsonlite::fromJSON(capability_json(), simplifyVector = FALSE)
  published <- vapply(manifest[["ok"]][["refusal_codes"]], function(row) row[["code"]], character(1))
  expect_true(condition[["code"]] %in% published)
})

test_that("both the specific and the general handler catch one refusal", {
  condition <- declined()

  expect_identical(
    tryCatch(stop(condition), plateforce_method_not_implemented = function(c) "specific"),
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
  expect_identical(condition[["code"]], "method_not_implemented")
})

# The fields the record carries, rather than the sentence a caller would have had to parse.
# The rule that could not be bound and the step it was named for are both fields.
test_that("an analysis that declined names the rule and the construct as fields", {
  condition <- declined()

  expect_identical(condition[["method_id"]], "onset.not.a.rule")
  expect_identical(condition[["slot"]], "movement_onset")
  expect_true(length(condition[["available"]]) > 0)
})

test_that("every field the idiom names is present on a refusal", {
  condition <- declined()
  carried <- names(unclass(condition))

  for (field in c("code", "method_id", "slot", "parameter", "value", "detail", "available")) {
    expect_true(field %in% carried, info = field)
  }
})

test_that("one trial analyses as many times as it is asked, without losing its trace", {
  # One trial analysed repeatedly is the shape a batch and an interactive session both
  # take. A trace reached through a pointer a collection can free would work once and fail
  # later, which no single-call test would see.
  trial <- quiet_trial(samples = 2400)
  weights <- numeric(0)
  for (index in seq_len(200)) {
    if (index %% 50L == 0L) gc(verbose = FALSE)
    result <- analyse_countermovement_jump(
      trial,
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    )
    weights <- union(weights, pf_value(result, "system_weight_newtons")@value)
  }

  expect_length(weights, 1L)
  expect_identical(trial@sample_count, 2400L)
})

# A rule that declines while the rest of the analysis computes is a partial result rather
# than a failed one, so it arrives on the result instead of being raised. Before the engine
# sent these across, it reached this package as a sentence in `warnings` and nothing else,
# and every code below was unraisable on any binding.
test_that("a landmark rule that placed nothing arrives as a code, not only a sentence", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.absolute_force",
    onset_parameters = list(threshold_n = 99999),
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_true(length(result@refusals) > 0)
  declined <- result@refusals[[1]]
  expect_true(inherits(declined, "plateforce_refusal"))
  expect_identical(declined[["code"]], "no_crossing")
  # The construct is the registry's own name for it, so a reader can look it up. `onset` is
  # the binding table's word and the registry declares no such construct.
  expect_identical(declined[["slot"]], "movement_onset")
  expect_identical(declined[["method_id"]], "onset.threshold.absolute_force")
  expect_identical(declined[["parameter"]], "threshold_n")
  expect_identical(declined[["value"]], 99999)

  # The same handler that catches a raised refusal reads this one.
  expect_identical(
    tryCatch(stop(declined), plateforce_no_crossing = function(c) c[["code"]]),
    "no_crossing"
  )
})

# The control. The same rule at a threshold the trace does reach declines nothing, so the
# assertion above is about the code rather than about every analysis carrying a refusal.
test_that("a rule that found its landmark declines nothing", {
  quiet <- rep(700, 1200) + rep(c(-0.4, 0.2, 0.4, -0.2), length.out = 1200)
  jump <- pf_trial(
    c(
      quiet,
      seq(700, 400, length.out = 360),
      seq(400, 1900, length.out = 360),
      rep(0, 600),
      rep(1700, 240)
    ),
    sample_rate_hz = 1200
  )
  result <- analyse_countermovement_jump(
    jump,
    weighing = "bwepoch.fixed_window",
    weighing_parameters = list(duration = 0.8),
    onset = "onset.threshold.absolute_force",
    onset_parameters = list(threshold_n = 20),
    takeoff = "takeoff.threshold.absolute_force"
  )
  expect_false(is.null(result@onset_index))
  expect_identical(length(result@refusals), 0L)
})

# A path this package cannot open used to raise `file_unreadable` and a file it read and
# could not get through used to raise `file_not_read`, two invented names for one failure,
# neither of them in the vocabulary this package's own manifest publishes.
test_that("a file that cannot be read raises the code the manifest publishes", {
  condition <- tryCatch(
    pf_read_force_file(
      file.path(tempdir(), "no-such-trace.txt"),
      sample_rate_hz = 1200,
      force_column = 1L,
      delimiter = "\t"
    ),
    plateforce_refusal = identity
  )
  expect_identical(condition[["code"]], "file_not_read")
  expect_true(inherits(condition, "plateforce_file_not_read"))
  manifest <- jsonlite::fromJSON(capability_json(), simplifyVector = FALSE)
  published <- vapply(manifest[["ok"]][["refusal_codes"]], function(row) row[["code"]], character(1))
  expect_true("file_not_read" %in% published)
})
