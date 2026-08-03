test_that("a trace with no jump in it reports no landmark and says which rule declined", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_null(result@onset_index)
  expect_null(result@takeoff_index)
  expect_true(is.na(pf_value(result, "jump_height_from_takeoff_meters")@value))
  expect_true(length(result@warnings) >= 2)
  expect_true(any(grepl("onset.threshold.noise_relative", result@warnings, fixed = TRUE)))
  expect_true(any(grepl("takeoff.threshold.absolute_force", result@warnings, fixed = TRUE)))
})

test_that("a value a rule did compute carries the rule and what it was bound to", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  weight <- pf_value(result, "system_weight_newtons")
  chain <- weight@provenance@depends_on

  expect_true(length(chain) >= 1)
  # What conditioned the signal is named before what read it, because every number below
  # was measured on the series the first rule produced.
  expect_identical(chain[[1]]@method_id, "filter.none")
  weighing <- link_named(chain, "bwepoch.fixed_window")
  expect_true(nrow(weighing@parameters) > 0)
  expect_true(all(weighing@parameters[["source"]] %in%
    c("stated", "assumed", "measured", "provisional")))
})

weighed <- function(...) {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force",
    ...
  )
  # By id rather than by position. Reading the first link assumed the weighing rule opened
  # the chain, which stopped being true the moment a rule ran before it.
  chain <- pf_value(result, "system_weight_newtons")@provenance@depends_on
  link_named(chain, "bwepoch.fixed_window")@parameters
}

source_of <- function(bound, name) bound[bound[["name"]] == name, "source"]

test_that("stating a value is what makes the record say the caller stated it", {
  # Two runs alike in everything except whether the caller stated the duration. Asserting
  # only that a stated value reads stated passes just as well when every value does.
  stated <- weighed(weighing_parameters = list(duration = 0.5))
  unstated <- weighed()

  expect_identical(source_of(stated, "duration"), "stated")
  expect_identical(source_of(unstated, "duration"), "assumed")
  expect_identical(source_of(stated, "centre"), source_of(unstated, "centre"))
  expect_false(source_of(unstated, "centre") == "stated")
})

test_that("a value the rule measured off this trace is not recorded as one it assumed", {
  bound <- weighed()

  # The weighing epoch's start is read from the trace rather than defaulted, and a record
  # that called it assumed would name a source no author chose.
  expect_identical(source_of(bound, "start_seconds"), "measured")
})

test_that("a dataset with no acquisition block fingerprints as incomplete", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_false(pf_value(result, "system_weight_newtons")@provenance@acquisition_complete)
})

test_that("the registry the numbers were bound to is named in the record", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  expect_identical(result@registry_digest, pf_registry()@digest)
})

test_that("a rule the caller named out of the registry is recorded as coming from it", {
  named <- c("bwepoch.fixed_window", "onset.threshold.noise_relative",
             "takeoff.threshold.absolute_force")
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = named[1], onset = named[2], takeoff = named[3]
  )

  ids <- vapply(result@bound_methods, function(m) as.character(m[["method_id"]]), character(1))
  backed <- vapply(result@bound_methods, function(m) isTRUE(m[["registry_backed"]]), logical(1))

  # Both sides in one run: the operators the rule composed were named by nobody, so a
  # surface reporting one value for every rule cannot satisfy both halves.
  expect_true(any(backed))
  expect_true(any(!backed))
  expect_identical(sort(unname(ids[backed])), sort(named))
  expect_true(all(ids[backed] %in% pf_registry()@method_ids))
})

# A rule for a construct other than the three the spine walks. Before the engine could
# dispatch by construct id, no surface could ask for one of these at all.
test_that("a rule named by its construct reports its number and the rule behind it", {
  jump <- pf_trial(
    c(
      rep(700, 1200) + rep(c(-0.4, 0.2, 0.4, -0.2), length.out = 1200),
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
    takeoff = "takeoff.threshold.absolute_force",
    derived = list(
      analysis_window = "window_end.takeoff.detected",
      peak_force = "force.peak.gross"
    )
  )

  peak <- pf_value(result, "peak_force_newtons")
  expect_true(peak@value > 0)
  expect_identical(peak@unit, "newtons")
  expect_identical(peak@provenance@method_id, "force.peak.gross")
  expect_identical(length(result@refusals), 0L)
})

# A rule that takes values reads them, and the value moves the number.
test_that("a value stated against a construct reaches its rule", {
  jump <- pf_trial(
    c(
      rep(700, 1200) + rep(c(-0.4, 0.2, 0.4, -0.2), length.out = 1200),
      seq(700, 400, length.out = 360),
      seq(400, 1900, length.out = 360),
      rep(0, 600),
      rep(1700, 240)
    ),
    sample_rate_hz = 1200
  )
  peak_at <- function(width) {
    result <- analyse_countermovement_jump(
      jump,
      weighing = "bwepoch.fixed_window",
      weighing_parameters = list(duration = 0.8),
      onset = "onset.threshold.absolute_force",
      onset_parameters = list(threshold_n = 20),
      takeoff = "takeoff.threshold.absolute_force",
      derived = list(
        analysis_window = "window_end.takeoff.detected",
        peak_force = list(
          method_id = "force.peak.estimator",
          parameters = list(averaging_window_seconds = width)
        )
      )
    )
    pf_value(result, "peak_force_newtons")@value
  }
  expect_true(peak_at(0.1) < peak_at(0))
})

# A construct forcing a choice nobody made declines by name rather than taking a window
# nobody picked, and the decline arrives with its code.
test_that("a peak asked for with no window chosen names the open choice", {
  jump <- pf_trial(
    c(
      rep(700, 1200) + rep(c(-0.4, 0.2, 0.4, -0.2), length.out = 1200),
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
    takeoff = "takeoff.threshold.absolute_force",
    derived = list(peak_force = "force.peak.gross")
  )
  expect_identical(length(result@refusals), 1L)
  expect_identical(result@refusals[[1]][["code"]], "decision_not_made")
  expect_true("analysis_window" %in% unlist(result@refusals[[1]][["available"]]))
  expect_false("peak_force_newtons" %in% names(result@values))
})
