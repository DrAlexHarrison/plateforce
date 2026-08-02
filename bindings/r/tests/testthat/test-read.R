test_that("a delimiter that was not declared is refused by naming the argument", {
  path <- tempfile(fileext = ".txt")
  writeLines(as.character(rep(700, 10)), path)
  on.exit(unlink(path))

  condition <- tryCatch(
    pf_read_force_file(path, sample_rate_hz = 100, force_column = 0),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_required_parameter_unstated")
  expect_identical(condition[["parameter"]], "delimiter")
})

test_that("a force column that was not declared is refused by naming the argument", {
  path <- tempfile(fileext = ".txt")
  writeLines(as.character(rep(700, 10)), path)
  on.exit(unlink(path))

  condition <- tryCatch(
    pf_read_force_file(path, sample_rate_hz = 100, delimiter = "\t"),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_required_parameter_unstated")
  expect_identical(condition[["parameter"]], "force_column")
})

test_that("a rate the file does not carry is refused rather than guessed", {
  path <- tempfile(fileext = ".txt")
  writeLines(as.character(rep(700, 10)), path)
  on.exit(unlink(path))

  condition <- tryCatch(
    pf_read_force_file(path, delimiter = "\t", force_column = 0),
    plateforce_refusal = identity
  )

  expect_s3_class(condition, "plateforce_required_parameter_unstated")
  expect_identical(condition[["parameter"]], "sample_rate_hz")
})

test_that("the read reports every choice it rested on", {
  path <- tempfile(fileext = ".txt")
  writeLines(paste("0.1", "1.1", as.character(rep(700, 10)), sep = "\t"), path)
  on.exit(unlink(path))

  trial <- pf_read_force_file(path, sample_rate_hz = 10, delimiter = "\t", force_column = 2)

  expect_identical(trial@read_report[["delimiter"]], "\t")
  expect_identical(trial@read_report[["force_column"]], 2L)
  expect_identical(trial@read_report[["rows_read"]], 10L)
  expect_identical(trial@read_report[["columns_per_row"]], 3L)
  expect_identical(trial@read_report[["blank_lines_skipped"]], 0L)
  expect_identical(trial@read_report[["sentinel_convention"]], "none")
})
