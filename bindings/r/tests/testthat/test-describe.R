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

test_that("every account states the unit its own value carries", {
  trace <- repository_trace()
  skip_if(is.null(trace), "the recorded trace is not beside this package")
  result <- analyse_countermovement_jump(
    pf_read_force_file(trace, sample_rate_hz = 1200, delimiter = "\t", force_column = 0),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  computed <- Filter(function(value) !is.na(value@value), result@values)
  described <- Filter(function(value) length(value@account) > 0L, result@values)

  # Reading fewer than most of the quantities would let the comparison below pass
  # having looked at almost nothing.
  expect_gt(length(computed), 5L)
  expect_identical(sort(names(described)), sort(names(computed)))

  for (name in names(described)) {
    value <- described[[name]]
    opening <- strsplit(value@account, "\n", fixed = TRUE)[[1]][1]
    expect_true(endsWith(opening, value@unit), info = name)
  }
})

test_that("the account names the choices the record beside it holds", {
  trace <- repository_trace()
  skip_if(is.null(trace), "the recorded trace is not beside this package")
  result <- analyse_countermovement_jump(
    pf_read_force_file(trace, sample_rate_hz = 1200, delimiter = "\t", force_column = 0),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )

  # A quantity is described by the engine from the chain behind it and the record beside it is
  # read off the same chain, so a reader who follows the sentence and a reader who queries the
  # object are looking at one set of decisions. They were two: the account read the rule's own
  # choices and the object was built with an empty choices frame, so on the recorded trial
  # every one of the eleven quantities named between four and eleven choices in prose and
  # carried none of them where a caller could reach them.
  described <- Filter(function(value) length(value@account) > 0L, result@values)
  expect_gt(length(described), 5L)

  disagreeing <- character(0)
  for (name in names(described)) {
    value <- described[[name]]
    lines <- strsplit(value@account, "\n", fixed = TRUE)[[1]]
    said <- trimws(grep(" = ", lines, fixed = TRUE, value = TRUE))
    held <- unlist(lapply(every_step(value@provenance), function(step) {
      if (!nrow(step@choices)) {
        return(character(0))
      }
      paste(step@choices[["name"]], "=", step@choices[["value"]])
    }))
    if (!setequal(said, held)) disagreeing <- c(disagreeing, name)
  }

  expect_identical(
    disagreeing, character(0),
    info = sprintf("%d of %d accounts name choices the record does not hold: %s",
                   length(disagreeing), length(described),
                   paste(disagreeing, collapse = ", "))
  )
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

# An account under a quantity that reports no number is its rule's own sentence about
# declining, and what it may not be is a claim that somebody measured something. The value and
# its unit open the first line wherever there is a value, so that line is where the claim is
# readable and the only place it is. Same property as the browser's, in
# `scripts/check-account.mjs`, and Python's, in `test_provenance.py`.
test_that("a quantity no rule computed gives no account claiming a measurement", {
  result <- analyse_countermovement_jump(
    quiet_trial(),
    weighing = "bwepoch.fixed_window",
    onset = "onset.threshold.noise_relative",
    takeoff = "takeoff.threshold.absolute_force"
  )
  height <- pf_value(result, "jump_height_from_takeoff_meters")

  expect_true(is.na(height@value))
  # Forbidding the account outright would hide the declining rule on exactly the quantities a
  # reader needs it named on, which is what this trial is: nothing moves, so the onset rule has
  # no band to search and says so.
  expect_identical(length(height@account), 1L)

  opens_on_a_figure <- function(name) {
    account <- pf_value(result, name)@account
    length(account) > 0L &&
      grepl("^\\s*-?[0-9]", strsplit(account, "\n", fixed = TRUE)[[1]][1])
  }
  quantities <- names(result@values)
  absent <- Filter(function(name) is.na(pf_value(result, name)@value), quantities)
  expect_gt(length(absent), 0L)

  # The same reading over the quantities that did produce a number, where every one has to
  # match. A reading that had stopped recognising a figure would report the line below clean
  # over any account at all, and this is the population that cannot let it.
  valued <- setdiff(quantities, absent)
  expect_gt(length(valued), 0L)
  expect_identical(Filter(function(name) !opens_on_a_figure(name), valued), character(0))

  claiming <- Filter(opens_on_a_figure, absent)
  expect_identical(
    claiming, character(0),
    info = sprintf("%d of %d quantities with no value open on a figure: %s",
                   length(claiming), length(absent), paste(claiming, collapse = ", "))
  )
})
