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

test_that("a real trial's onset sweep is dominated by one rule, and nothing warns", {
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

  values <- vapply(sweep[["variants"]], function(v) {
    if (is.null(v[["value"]])) NA_real_ else as.double(v[["value"]])
  }, numeric(1))
  expect_identical(sweep[["failed"]], 0L)

  # Four of the five agree to within a few millimetres and the fifth is a third of a metre
  # away, so the headline spread reports a rule that found the wrong event rather than a
  # disagreement between rules. The engine reports no failure for it, which is the state
  # the quality signal is being built to change.
  distances <- abs(values - stats::median(values, na.rm = TRUE))
  expect_identical(sum(distances > 0.1, na.rm = TRUE), 1L)
  expect_gt(sweep[["spread_absolute"]], 0.1)
})
