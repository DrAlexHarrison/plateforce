# `d$takeoff_velocity` returns `takeoff_velocity_meters_per_second`'s value on a plain
# data frame with no warning at all, and this package's column names are long and unit
# carrying, which makes a short prefix more tempting rather than less.

test_that("a relation's column is read by its whole name everywhere in this package", {
  roots <- c(
    testthat::test_path("..", "..", "R"),
    testthat::test_path("..", "..", "vignettes"),
    testthat::test_path("..", "..", "man")
  )
  roots <- roots[dir.exists(roots)]
  skip_if(length(roots) == 0, "the sources are not beside this test")

  relations <- c("census", "parameters", "choices", "results", "provenance", "refusals",
                 "run", "bindings", "entry", "read_report")
  offenders <- character(0)
  for (root in roots) {
    files <- list.files(root, pattern = "[.](R|Rmd|Rd)$", full.names = TRUE)
    for (file in files) {
      lines <- readLines(file, warn = FALSE)
      for (relation in relations) {
        hits <- grep(paste0("\\b", relation, "\\$"), lines)
        if (length(hits)) {
          offenders <- c(offenders, sprintf("%s:%d", basename(file), hits))
        }
      }
    }
  }

  expect_identical(offenders, character(0))
})
