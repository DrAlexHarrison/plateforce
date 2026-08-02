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

  expect_s3_class(condition, "plateforce_slot_has_no_rules")
  expect_identical(condition[["slot"]], "landing")
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
