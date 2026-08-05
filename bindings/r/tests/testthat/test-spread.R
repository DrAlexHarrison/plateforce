test_that("a sweep names every alternative it ran and takes no option to enable it", {
  standing <- quiet_trial()
  sweep <- pf_spread(
    standing,
    quantity = "system_weight_newtons",
    slot = "weighing",
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  bindings <- pf_bindings()
  expect_identical(
    sweep[["combinations_run"]],
    sum(bindings[["slot"]] == "weighing")
  )
  expect_identical(
    sweep[["succeeded"]] + sweep[["failed"]],
    sweep[["combinations_run"]]
  )
  expect_identical(sweep[["quantity_key"]], "system_weight_newtons")
})

test_that("a slot this build runs no rule for is refused with the ones it does", {
  condition <- tryCatch(
    pf_spread(
      quiet_trial(),
      quantity = "system_weight_newtons",
      slot = "landing",
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    ),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_slot_offers_no_alternative")
  expect_identical(condition[["slot"]], "landing")
  expect_identical(
    conditionMessage(condition),
    "this analysis runs no rule for landing, so there is nothing to sweep"
  )
})

# The terminal and the notebook refuse a step this build runs one rule for, in one sentence.
# This surface let the name through, and the engine then refused it as a name that is not one
# of the axes a sweep can vary. A different fault from the one the caller has: it sends a
# reader looking for a typo or a binding they forgot, and never says the step is reached one
# way in this build.
#
# The step is read off the binding table rather than written here, so a rule added for it
# moves this test to a step that still holds one rather than leaving it asserting on a
# population that has moved on.
test_that("a slot this build runs one rule for is refused in the terminal's own sentence", {
  bindings <- pf_bindings()
  held_once <- names(Filter(function(count) count == 1L, table(bindings[["slot"]])))
  skip_if(!length(held_once), "this build runs two or more rules for every step")
  step <- held_once[[1]]

  condition <- tryCatch(
    pf_spread(
      quiet_trial(),
      quantity = "system_weight_newtons",
      slot = step,
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    ),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_slot_offers_no_alternative")
  expect_identical(
    conditionMessage(condition),
    paste0("this analysis runs one rule for ", step, ", so there is nothing to sweep")
  )
})

test_that("several steps sweep every combination of their rules", {
  standing <- quiet_trial()
  bindings <- pf_bindings()
  steps <- c("weighing", "onset")

  sweep <- pf_spread(
    standing,
    quantity = "system_weight_newtons",
    slot = steps,
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  # The product of the two, read off the binding table. Sweeping them one at a time reports
  # neither the widest disagreement nor the narrowest, which is why one call takes both.
  expected <- sum(bindings[["slot"]] == "weighing") * sum(bindings[["slot"]] == "onset")
  expect_gt(expected, sum(bindings[["slot"]] == "weighing"))
  expect_identical(sweep[["combinations_run"]], expected)
  expect_identical(length(sweep[["variants"]]), expected)
  expect_identical(sweep[["succeeded"]] + sweep[["failed"]], expected)
})

test_that("one step is one axis, whichever keyboard names it twice", {
  condition <- tryCatch(
    pf_spread(
      quiet_trial(),
      quantity = "system_weight_newtons",
      slot = c("onset", "onset"),
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    ),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_sweep_axes_not_understood")
  expect_identical(
    conditionMessage(condition),
    "'onset' is named twice, and one step is one axis"
  )
})

test_that("a rule list or a parameter describes one step, so several is refused", {
  for (stated in list(
    list(method_ids = c("bwepoch.fixed_window", "bwepoch.manual_placement")),
    list(parameter = "k", values = c(2.0, 5.0))
  )) {
    condition <- tryCatch(
      do.call(pf_spread, c(list(
        quiet_trial(),
        quantity = "system_weight_newtons",
        slot = c("weighing", "onset"),
        weighing = "bwepoch.fixed_window",
        onset = "onset.threshold.noise_relative",
        takeoff = "takeoff.threshold.absolute_force"
      ), stated)),
      plateforce_refusal = identity
    )

    expect_s3_class(condition, "plateforce_sweep_axes_not_understood")
    expect_identical(
      conditionMessage(condition),
      "parameter and method_ids each describe one step, so name one step or neither"
    )
  }
})

# A length-one atomic vector is written as a scalar, so one value to sweep left here as a
# number where the engine reads a list of them. The fault came back naming a column of the
# request, which is a sentence about JSON rather than about the value the caller typed.
test_that("a parameter sweep states its values as a list, one value or several", {
  standing <- quiet_trial()
  swept <- function(values) {
    pf_spread(
      standing,
      quantity = "system_weight_newtons",
      slot = "weighing",
      parameter = "duration",
      values = values,
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    )
  }

  expect_identical(swept(c(0.5, 1.0))[["combinations_run"]], 2L)
  expect_identical(swept(0.5)[["combinations_run"]], 1L)
})

test_that("a variant that declined is listed with its reason rather than dropped", {
  sweep <- pf_spread(
    quiet_trial(),
    quantity = "jump_height_from_takeoff_meters",
    slot = "onset",
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_identical(length(sweep[["variants"]]), sweep[["combinations_run"]])
  for (variant in sweep[["variants"]]) {
    expect_true(nzchar(variant[["label"]]))
    expect_true(!is.null(variant[["value"]]) || !is.null(variant[["failure_reason"]]))
  }
})

test_that("every onset rule this build runs computes, and they disagree by a real amount", {
  trace <- repository_trace()
  skip_if(is.null(trace), "the trial these fixtures were derived from is not here")
  trial <- pf_read_force_file(trace, sample_rate_hz = 1200, delimiter = "\t",
                              force_column = 0, sentinel_convention = "none")

  sweep <- pf_spread(
    trial,
    quantity = "jump_height_from_takeoff_meters",
    slot = "onset",
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  values <- vapply(sweep[["variants"]], function(one) {
    if (is.null(one[["value"]])) NA_real_ else as.double(one[["value"]])
  }, numeric(1))

  expect_length(values, as.integer(fixture_field("onset-sweep.txt", "variants")))
  expect_identical(sweep[["failed"]], as.integer(fixture_field("onset-sweep.txt", "failed")))
  expect_equal(
    sweep[["spread_absolute"]],
    as.double(fixture_field("onset-sweep.txt", "spread_absolute_meters"))
  )

  # No rule sits a long way from the rest, so the spread is five rules disagreeing rather
  # than one of them finding the wrong event. The two are the same number to a reader and
  # different findings to this project.
  furthest <- max(abs(values - stats::median(values, na.rm = TRUE)), na.rm = TRUE)
  expect_equal(furthest, as.double(fixture_field("onset-sweep.txt", "furthest_from_median_meters")))
  expect_lt(furthest, 0.1)
  expect_gt(sweep[["spread_absolute"]], 0)
})
