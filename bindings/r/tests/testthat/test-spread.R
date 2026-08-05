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

# A word the binding table holds no rule for. Written as `landing` this named a construct
# that later had two rules bound to it, so R built the axis and the engine refused it as one
# the request does not carry: a different sentence, and a green line turning red on a data
# edit nobody connected to it. The test below is the case `landing` moved into.
test_that("a step this build runs no rule for is refused with the ones it does", {
  step <- "not_a_step_this_build_runs"
  expect_false(step %in% pf_bindings()[["slot"]])

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
  expect_identical(condition[["slot"]], step)
  expect_identical(
    conditionMessage(condition),
    paste0("this analysis runs no rule for ", step, ", so there is nothing to sweep")
  )
})

# A construct the build runs rules for and this request did not bind is not an axis: sweeping
# it would run a rule nobody chose. R passes the name through and the engine refuses it with
# the axes this request does carry, which is the answer the terminal and the notebook give.
test_that("a construct this request did not bind is refused with the axes it did", {
  bindings <- pf_bindings()
  landmarks <- c("weighing", "onset", "takeoff")
  # Two rules or more, or this surface refuses the step before the engine reads the axis and
  # the test would assert on a sentence from the wrong side of the wire.
  counted <- table(bindings[["slot"]])
  unbound <- setdiff(names(counted)[counted > 1], landmarks)
  skip_if(!length(unbound), "every construct with alternatives is one this request binds")

  condition <- tryCatch(
    pf_spread(
      quiet_trial(),
      quantity = "system_weight_newtons",
      slot = unbound[[1]],
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    ),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_unknown_parameter")
  expect_true(all(landmarks %in% condition[["available"]]))
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

  expect_s3_class(condition, "plateforce_value_not_accepted")
  expect_identical(
    conditionMessage(condition),
    "'onset' is named twice, and one step is one axis"
  )
})

test_that("a vector of method_ids describes one step, so several is refused", {
  condition <- tryCatch(
    pf_spread(
      quiet_trial(),
      quantity = "system_weight_newtons",
      slot = c("weighing", "onset"),
      method_ids = c("bwepoch.fixed_window", "bwepoch.manual_placement"),
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    ),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_value_not_accepted")
  expect_identical(
    conditionMessage(condition),
    "a vector of method_ids describes one step, so name one step or key the ids by step"
  )
})

# Keyed by step, the ids say which rules belong to which, so named rules on two steps is one
# call. As a vector that question cannot be asked at all.
test_that("method_ids keyed by step names the rules for each of them", {
  bindings <- pf_bindings()
  weighing_rules <- bindings[["id"]][bindings[["slot"]] == "weighing"][1:2]
  onset_rules <- bindings[["id"]][bindings[["slot"]] == "onset"][1:2]

  sweep <- pf_spread(
    quiet_trial(),
    quantity = "system_weight_newtons",
    method_ids = list(weighing = weighing_rules, onset = onset_rules),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_identical(sweep[["combinations_run"]], 4L)
  expect_identical(length(sweep[["axes_varied"]]), 2L)
})

# A length-one atomic vector is written as a scalar, so one value to sweep left here as a
# number where the engine reads a list of them. The fault came back naming a column of the
# request, which is a sentence about JSON rather than about the value the caller typed.
test_that("a setting sweep states its values as a list, one value or several", {
  standing <- quiet_trial()
  swept <- function(values) {
    pf_spread(
      standing,
      quantity = "system_weight_newtons",
      vary = list("weighing.duration" = values),
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    )
  }

  expect_identical(swept(c(0.5, 1.0))[["combinations_run"]], 2L)
  expect_identical(swept(0.5)[["combinations_run"]], 1L)
  # One combination holds no disagreement, so it publishes no spread. Reported as 0.0 it
  # said every alternative agreed, over a set nobody compared.
  expect_null(swept(0.5)[["spread_absolute"]])
  expect_false(is.null(swept(c(0.5, 1.0))[["spread_absolute"]]))
})

# The engine sweeps a rule and a value inside it on one request, and no surface could state
# that. On subject 01 trial 1 the six published values of onset.k move a jump height
# 0.01981 m against 0.01924 m for the five onset rules, so a reader asking about a number
# resting on both is asking one question.
test_that("the rules and a value inside them vary on one call", {
  standing <- quiet_trial()
  bindings <- pf_bindings()
  weighing_rules <- sum(bindings[["slot"]] == "weighing")

  both <- pf_spread(
    standing,
    quantity = "system_weight_newtons",
    slot = "weighing",
    vary = list("weighing.duration" = c(0.5, 1.0)),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_identical(both[["combinations_run"]], weighing_rules * 2L)
  expect_identical(length(both[["axes_varied"]]), 2L)
  # The record names both, so a reader of the figure can see the whole set it came from.
  varied <- vapply(both[["axes_varied"]], function(axis) {
    if (is.null(axis[["parameter"]])) "rules" else axis[["parameter"]]
  }, character(1))
  expect_setequal(varied, c("rules", "duration"))
})

test_that("one setting is one axis, whichever way it is named twice", {
  condition <- tryCatch(
    pf_spread(
      quiet_trial(),
      quantity = "system_weight_newtons",
      vary = list("weighing.duration" = c(0.5, 0.5)),
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    ),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_value_not_accepted")
  expect_identical(
    conditionMessage(condition),
    "weighing.duration names 0.5 twice, and one value is one variant"
  )
})

test_that("naming no step and no setting is refused rather than swept", {
  condition <- tryCatch(
    pf_spread(
      quiet_trial(),
      quantity = "system_weight_newtons",
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    ),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_required_parameter_unstated")
  expect_identical(
    conditionMessage(condition),
    "no step and no setting were named, so there is nothing to sweep"
  )
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

  # Which rule produced which number, rather than the summary alone. Held to four summary
  # figures the fixture certified the arithmetic and said nothing about the record, which is
  # the half of this product the arithmetic is in service of: a sweep that renamed every rule
  # and returned the same five numbers in the same order satisfied every line above.
  ran <- vapply(sweep[["variants"]], function(one) one[["settings"]][[1]][[2]], character(1))
  recorded <- strsplit(fixture_field("onset-sweep.txt", "method_ids"), ",", fixed = TRUE)[[1]]
  expect_identical(ran, recorded)
  expect_true(all(nzchar(recorded)))
  expect_identical(length(unique(recorded)), length(recorded))

  # Each rule against the number it produced, paired by position, so a reordering of the
  # document is a failure rather than a relabelling nobody sees.
  expect_equal(
    values,
    as.double(strsplit(fixture_field("onset-sweep.txt", "values_meters"), ",",
                       fixed = TRUE)[[1]])
  )

  # The registry these numbers came out of, as the registry declares itself. A record that
  # names no registry is a number a reader cannot place.
  expect_identical(
    if (is.null(sweep[["registry_declared_version"]])) "none" else
      sweep[["registry_declared_version"]],
    fixture_field("onset-sweep.txt", "registry_declared_version")
  )

  # No rule sits a long way from the rest, so the spread is five rules disagreeing rather
  # than one of them finding the wrong event. The two are the same number to a reader and
  # different findings to this project.
  furthest <- max(abs(values - stats::median(values, na.rm = TRUE)), na.rm = TRUE)
  expect_equal(furthest, as.double(fixture_field("onset-sweep.txt", "furthest_from_median_meters")))
  expect_lt(furthest, 0.1)
  expect_gt(sweep[["spread_absolute"]], 0)
})
