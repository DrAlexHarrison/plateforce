# A block that cannot be filled fingerprints as incomplete rather than as matching, and
# the members it holds are named by the block itself.

test_that("the members this package asks for are the members the block declares", {
  # Naming them again here would be a second list, and the second list is the one that
  # goes stale: a member that arrives upstream would be one R never asks for.
  asked <- setdiff(names(formals(pf_acquisition)), "")

  expect_identical(asked, pf_acquisition_members())
})

test_that("a trial with no acquisition block names every member still to find", {
  trial <- quiet_trial()

  expect_false(trial@acquisition_complete)
  expect_identical(trial@acquisition_missing, pf_acquisition_members())
})

test_that("a partial block is incomplete and names what is missing", {
  trial <- pf_trial(
    rep(700, 1200),
    sample_rate_hz = 1200,
    acquisition = pf_acquisition(tare_state = "tared_before_trial")
  )

  expect_false(trial@acquisition_complete)
  expect_identical(trial@acquisition_missing, setdiff(pf_acquisition_members(), "tare_state"))
})

filled_trial <- function() {
  pf_trial(
    rep(700, 1200),
    sample_rate_hz = 1200,
    acquisition = pf_acquisition(
      filter_at_capture = "none",
      tare_state = "tared_before_trial",
      plate_natural_frequency_hz = 400,
      floor_surface = "concrete",
      firmware_version = "2.4.1"
    )
  )
}

test_that("a filled block is complete and nothing is left to find", {
  trial <- filled_trial()

  expect_true(trial@acquisition_complete)
  expect_identical(trial@acquisition_missing, character(0))
})

test_that("whether the block was filled is what the record and the account say", {
  weight_of <- function(trial) {
    result <- analyse_countermovement_jump(
      trial,
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    )
    pf_value(result, "system_weight_newtons")
  }

  filled <- weight_of(filled_trial())
  unfilled <- weight_of(quiet_trial())

  expect_true(filled@provenance@acquisition_complete)
  expect_false(unfilled@provenance@acquisition_complete)
  expect_false(any(grepl("acquisition block incomplete", filled@account, fixed = TRUE)))
  expect_true(any(grepl("acquisition block incomplete", unfilled@account, fixed = TRUE)))
})
