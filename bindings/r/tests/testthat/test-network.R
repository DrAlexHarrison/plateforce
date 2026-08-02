# This package makes no outbound request. The guard is the check rather than a sentence
# in a document, because a sentence does not fail when somebody adds one.

test_that("no file under R/ reaches a network", {
  root <- testthat::test_path("..", "..", "R")
  if (!dir.exists(root)) root <- system.file("R", package = "plateforce")
  skip_if_not(dir.exists(root), "the sources are not beside this test")

  reaching <- c("url\\(", "download\\.file", "curl", "httr", "socketConnection",
                "readLines\\(\\s*\"https?://", "nsl\\(")
  offenders <- character(0)
  for (file in list.files(root, pattern = "[.]R$", full.names = TRUE)) {
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
