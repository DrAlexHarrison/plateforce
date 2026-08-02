# Every compiled entry point this package calls is one the shipped library registered.
# The wrappers are written here by hand, and a hand-written call that no longer matches
# the library is a call that fails at the moment a user makes it rather than at build.

test_that("every compiled entry point the package calls is registered by the library", {
  registered <- names(getDLLRegisteredRoutines("plateforce")[[".Call"]])
  called <- grep("^wrap__", ls(asNamespace("plateforce"), all.names = TRUE), value = TRUE)

  root <- testthat::test_path("..", "..", "R")
  skip_if_not(dir.exists(root), "the sources are not beside this test")
  text <- unlist(lapply(list.files(root, pattern = "[.]R$", full.names = TRUE),
                        readLines, warn = FALSE))
  named <- unique(unlist(regmatches(text, gregexpr("wrap__[A-Za-z0-9_]+", text))))

  expect_true(length(named) > 0)
  expect_true(all(named %in% registered),
              info = paste(setdiff(named, registered), collapse = ", "))
})
