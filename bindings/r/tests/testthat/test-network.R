# This package makes no outbound request. The guard is the check rather than a sentence
# in a document, because a sentence does not fail when somebody adds one.

test_that("no file under R/ reaches a network", {
  candidates <- c(
    testthat::test_path("..", "..", "R"),
    testthat::test_path("..", "..", "..", "..", "bindings", "r", "R"),
    file.path("R"),
    file.path("bindings", "r", "R")
  )
  roots <- candidates[dir.exists(candidates)]
  files <- unlist(lapply(roots, list.files, pattern = "[.]R$", full.names = TRUE))
  skip_if_not(length(files) > 0, "the R sources are not beside this test")

  reaching <- c("url\\(", "download\\.file", "curl", "httr", "socketConnection",
                "readLines\\(\\s*\"https?://", "nsl\\(")
  offenders <- character(0)
  for (file in files) {
    lines <- readLines(file, warn = FALSE)
    for (pattern in reaching) {
      hits <- grep(pattern, lines)
      if (length(hits)) {
        offenders <- c(offenders, sprintf("%s:%d", basename(file), hits))
      }
    }
  }

  expect_identical(offenders, character(0))
})
