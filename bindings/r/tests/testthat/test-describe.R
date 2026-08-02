test_that("the account names the value, its rule, and every rule upstream of it", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  weight <- pf_value(result, "system_weight_newtons")
  account <- capture.output(describe(weight))

  expect_match(account[1], "700")
  expect_match(account[1], "newtons")
  expect_true(any(grepl("bwepoch.fixed_window", account, fixed = TRUE)))
  expect_true(any(grepl("acquisition block incomplete", account, fixed = TRUE)))
})

test_that("no file in this package assembles the sentence", {
  root <- testthat::test_path("..", "..", "R")
  skip_if_not(dir.exists(root), "the sources are not beside this test")

  assembling <- c("paste0?\\(.*method_id", "sprintf\\(.*method_id", "format\\(.*meters")
  offenders <- character(0)
  for (file in list.files(root, pattern = "[.]R$", full.names = TRUE)) {
    lines <- readLines(file, warn = FALSE)
    for (pattern in assembling) {
      hits <- grep(pattern, lines)
      if (length(hits)) offenders <- c(offenders, sprintf("%s:%d", basename(file), hits))
    }
  }

  expect_identical(offenders, character(0))
})

test_that("a quantity no rule computed carries no account rather than an invented one", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  height <- pf_value(result, "jump_height_from_takeoff_meters")

  expect_true(is.na(height@value))
  expect_identical(length(height@account), 0L)
})
