withr::local_options(
  list(warnPartialMatchDollar = TRUE),
  .local_envir = testthat::teardown_env()
)
