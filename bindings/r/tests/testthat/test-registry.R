test_that("the census reports each population apart and never sums them", {
  census <- pf_registry()@census

  expect_false("total" %in% names(census))
  expect_false(any(census[["population"]] %in% c("total", "all")))
  expect_setequal(
    census[["population"]],
    c("constructs", "computation_entries", "protocol_entries")
  )
})

test_that("a derived count appears only on the population it was taken over", {
  census <- pf_registry()@census
  other <- census[census[["population"]] != "computation_entries", , drop = FALSE]

  expect_true(all(is.na(other[["genuine_debates"]])))
  expect_true(all(is.na(other[["can_find_wrong_event"]])))

  computation <- census[census[["population"]] == "computation_entries", , drop = FALSE]
  expect_false(is.na(computation[["genuine_debates"]]))
  expect_false(is.na(computation[["can_find_wrong_event"]]))
  expect_lte(computation[["genuine_debates"]], computation[["count"]])
  expect_lte(computation[["can_find_wrong_event"]], computation[["count"]])
})

test_that("the census R reports is the census the engine counted", {
  recorded <- fixture_lines("census.txt")
  census <- pf_registry()@census
  live <- vapply(seq_len(nrow(census)), function(row) {
    paste(census[["population"]][row], census[["count"]][row],
          census[["genuine_debates"]][row], census[["can_find_wrong_event"]][row])
  }, character(1))

  expect_identical(live, recorded)
})

test_that("an entry comes back with the fields the registry states", {
  entry <- pf_entry("onset.threshold.noise_relative")

  expect_identical(entry[["id"]], "onset.threshold.noise_relative")
  expect_identical(entry[["construct"]], "movement_onset")
  expect_true(length(entry[["parameter"]]) > 0)
})

test_that("an id no entry carries is refused by name", {
  condition <- tryCatch(pf_entry("onset.not.a.rule"), plateforce_refusal = identity)

  expect_s3_class(condition, "plateforce_method_not_in_registry")
  expect_identical(condition[["method_id"]], "onset.not.a.rule")
})

test_that("every rule this build runs names an id and a construct", {
  bindings <- pf_bindings()

  expect_true(nrow(bindings) > 0)
  expect_false(any(is.na(bindings[["id"]])))
  expect_false(any(is.na(bindings[["construct"]])))
})
