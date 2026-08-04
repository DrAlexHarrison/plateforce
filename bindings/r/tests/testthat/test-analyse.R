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
  weighing <- pf_value(result, "system_weight_newtons")@provenance

  # The system weight is the weighing rule's own answer, so that rule roots the record rather
  # than sitting a step under a root that names no arithmetic.
  expect_identical(weighing@method_id, "bwepoch.fixed_window")
  expect_true(nrow(weighing@parameters) > 0)
  expect_true(all(weighing@parameters[["source"]] %in%
    c("stated", "assumed", "measured", "provisional")))

  # What conditioned the signal sits under it, because the number was measured on the series
  # that rule produced.
  expect_identical(link_named(weighing@depends_on, "filter.none")@method_id, "filter.none")
})

# Every value the weighing rule read, quantities and named choices alike, each beside where it
# came from. A reader asking where one name came from is not asking which of the two it is.
weighed <- function(...) {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force",
    ...
  )
  record <- pf_value(result, "system_weight_newtons")@provenance
  rbind(record@parameters, record@choices)
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

test_that("the revision a caller pinned and the one the registry claims are two fields", {
  # This surface carried neither until 2026-08-03: its document omitted registry_version
  # entirely, so no R session could say which revision of the data produced a number, and
  # the terminal meanwhile published the registry's own claim under the pin's name.
  declared <- pf_registry()@declared_version
  expect_true(length(declared) == 1L && nzchar(declared))

  pin <- "wsrp-not-a-revision-any-registry-declares"
  expect_false(identical(pin, declared))

  run <- function(version) {
    analyse_countermovement_jump(
      quiet_trial(),
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force",
      registry_version = version
    )
  }

  unpinned <- run(NULL)
  expect_identical(unpinned@registry_version, character(0))
  expect_identical(unpinned@registry_declared_version, declared)

  pinned <- run(pin)
  expect_identical(pinned@registry_version, pin)
  expect_identical(pinned@registry_declared_version, declared)
  expect_identical(pinned@registry_digest, unpinned@registry_digest)

  # And on the record each number carries, not on the run alone. A field filled only where
  # it is asserted passes an assertion made in that one place.
  record <- pf_value(pinned, "jump_height_from_takeoff_meters")@provenance
  steps <- every_step(record)

  # Every depth, not the rules one step under the root. The control first: a walk that stopped
  # reaching the tree would satisfy every line below by looking at nothing.
  expect_gt(length(steps), 1L)
  for (link in steps) {
    expect_identical(link@registry_version, pin)
    expect_identical(link@registry_declared_version, declared)
  }
})

test_that("a rule the registry carries is recorded as coming from it, named or composed", {
  named <- c("bwepoch.fixed_window", "onset.threshold.noise_relative",
             "takeoff.threshold.absolute_force")
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = named[1], onset = named[2], takeoff = named[3]
  )

  ids <- vapply(result@bound_methods, function(m) as.character(m[["method_id"]]), character(1))
  backed <- vapply(result@bound_methods, function(m) isTRUE(m[["registry_backed"]]), logical(1))
  carried <- ids %in% pf_registry()@method_ids

  # The control. Every equality below holds trivially over no bound methods, and the run
  # has to bind more than the caller named for the composed half to be asserted at all.
  expect_gt(length(ids), length(named))

  # The binding composes operators onto the rule the caller chose and a caller never types
  # those, yet each is an entry in its own right carrying its own citation. Reporting one as
  # absent from the registry it is filed in is what this asserts against: what decides the
  # flag is whether the registry carries the id, never whether the caller typed it.
  composed <- setdiff(ids, named)
  expect_true(length(composed) > 0)
  expect_true(all(composed %in% pf_registry()@method_ids))

  # `vapply` keeps the names of the list it walked and `%in%` returns none, so the two are
  # compared on their values alone rather than on an attribute neither side is asserting.
  expect_identical(unname(backed), carried)
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


# What the rule that conditioned the signal recorded, and where each value it read came from.
#
# The phase runs on every analysis and this package had no argument for it, so every R session
# reported the software's answer about the signal its numbers were measured on.
conditioned <- function(...) {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force",
    ...
  )
  result@bound_methods[["filter.none"]]
}

edge_source_of <- function(bound) {
  as.character(bound[["parameter_sources"]][["passband_edge"]])
}

test_that("an R session states what conditioned the signal and the record names the caller", {
  stated <- conditioned(
    conditioning = list(
      conditioned_force_signal = list(options = list(passband_edge = "none"))
    )
  )
  unstated <- conditioned()

  expect_identical(edge_source_of(stated), "stated")
  expect_identical(edge_source_of(unstated), "assumed")
  expect_length(stated[["unread_parameters"]], 0)
})

test_that("naming the rule the phase runs anyway is recorded as the session's own choice", {
  named <- conditioned(
    conditioning = list(conditioned_force_signal = "filter.none")
  )
  unnamed <- conditioned()

  expect_identical(as.character(named[["method_source"]]), "stated")
  expect_identical(as.character(unnamed[["method_source"]]), "assumed")

  # Held equal on the one field under test, so anything else that moved is reported here
  # rather than being covered by the difference this expects.
  levelled <- named
  levelled[["method_source"]] <- unnamed[["method_source"]]
  expect_identical(levelled, unnamed)
})

test_that("an edge this rule does not take is refused with the one it does", {
  # `filter.none` reports the recording as it was digitised, so a session asking it for a
  # 20 Hz passband is asking it for a filter, and answering `none` would write a word into
  # their record they did not choose.
  declined <- tryCatch(
    analyse_countermovement_jump(
      quiet_trial(),
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force",
      conditioning = list(
        conditioned_force_signal = list(options = list(passband_edge = "20"))
      )
    ),
    plateforce_refusal = identity
  )

  expect_true(inherits(declined, "plateforce_refusal"))
  expect_identical(declined[["code"]], "value_not_accepted")
  expect_identical(declined[["method_id"]], "filter.none")
  expect_identical(declined[["parameter"]], "passband_edge")
  expect_identical(as.character(unlist(declined[["available"]])), "none")
})

test_that("a construct this build conditions nothing with is refused by name", {
  declined <- tryCatch(
    analyse_countermovement_jump(
      quiet_trial(),
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force",
      conditioning = list(movement_onset = "filter.none")
    ),
    plateforce_refusal = identity
  )

  expect_true(inherits(declined, "plateforce_refusal"))
  expect_identical(
    as.character(unlist(declined[["available"]])),
    "conditioned_force_signal"
  )
})
