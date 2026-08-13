# The rule is that every population is reported under its own name and none of them is ever
# summed, however many of them there are. A list of the populations that existed on the day
# this was written asserts a snapshot instead, and reds the moment a fourth is added for
# being correct, which is what happened when presets arrived.
test_that("the census reports each population apart and never sums them", {
  census <- pf_registry()@census

  expect_false("total" %in% names(census))
  expect_false(any(census[["population"]] %in% c("total", "all")))

  # The control. An empty census satisfies every assertion below about what it must not do.
  expect_gt(nrow(census), 1)

  populations <- census[["population"]]
  expect_false(any(duplicated(populations)))
  expect_true(all(nzchar(populations)))

  # No row is any other row's total, which is what summing would produce whatever the
  # populations are called. Checked against every subset of two or more rather than against
  # the whole, so a sum of some of them is caught as well as a sum of all.
  counts <- census[["count"]]
  expect_true(all(counts > 0))
  for (row in seq_along(counts)) {
    others <- counts[-row]
    # `seq` counts backwards when its end is below its start, so a census of two populations
    # would ask for every subset of size one and then error rather than assert anything.
    if (length(others) < 2) next
    for (size in seq.int(2, length(others))) {
      sums <- utils::combn(others, size, sum)
      expect_false(counts[row] %in% sums,
                   info = sprintf("%s equals the sum of %d other populations",
                                  populations[row], size))
    }
  }
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

test_that("the census R reports is the census the command line counts", {
  repository <- testthat::test_path("..", "..", "..", "..")
  skip_if_not(dir.exists(file.path(repository, "crates", "plateforce-cli")),
              "the command line is not beside this test")
  skip_if_not(nzchar(Sys.which("cargo")), "no cargo on this machine")

  target <- file.path(tempdir(), "plateforce-cli-target")
  printed <- suppressWarnings(system2(
    "cargo",
    c("run", "-q", "--offline", "--locked",
      "--manifest-path", shQuote(file.path(repository, "Cargo.toml")),
      "-p", "plateforce-cli", "--",
      "--registry", shQuote(file.path(repository, "registry")), "registry", "census"),
    env = c(paste0("CARGO_TARGET_DIR=", target), "CARGO_NET_OFFLINE=true"),
    stdout = TRUE, stderr = FALSE
  ))
  expect_true(any(startsWith(trimws(printed), "constructs")),
              info = paste(printed, collapse = " | "))

  number <- function(label) {
    line <- printed[startsWith(trimws(printed), label)]
    skip_if(length(line) == 0, paste(label, "is not in the census"))
    # The derived rows read "N of M". The count is the first number and M is the
    # denominator it was taken over, so taking the last number would report the wrong one.
    as.integer(regmatches(line[1], gregexpr("[0-9]+", line[1]))[[1]][1])
  }

  census <- pf_registry(file.path(repository, "registry"))@census
  row <- function(population) census[census[["population"]] == population, , drop = FALSE]

  expect_identical(row("constructs")[["count"]], number("constructs"))
  expect_identical(row("computation_entries")[["count"]], number("computation entries"))
  expect_identical(row("computation_entries")[["genuine_debates"]],
                   number("of which genuine debates"))
  expect_identical(row("computation_entries")[["can_find_wrong_event"]],
                   number("of which can find the wrong event"))
  expect_identical(row("protocol_entries")[["count"]], number("protocol entries"))
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

# The revision a registry names about itself is not recoverable from its digest, which is why
# a result has to carry it. The walk that assembles entries and measures the digest reads the
# toml files alone, and the revision sits beside them in a VERSION file.
test_that("the declared revision is a separate fact from the digest", {
  shipped <- pf_registry()
  expect_true(length(shipped@declared_version) == 1L && nzchar(shipped@declared_version))
  expect_false(identical(shipped@declared_version, shipped@digest))

  # A copy of the shipped rules under a different declared revision. Same bytes among the
  # rules, so the digest must not move; different VERSION, so the claim must.
  root <- file.path(tempfile("plateforce-registry-"), "registry")
  dir.create(root, recursive = TRUE)
  file.copy(list.files(shipped@root, full.names = TRUE), root, recursive = TRUE)
  writeLines("wsrp-a-revision-nobody-shipped", file.path(root, "VERSION"))

  renamed <- pf_registry(root)
  expect_identical(renamed@digest, shipped@digest)
  expect_identical(renamed@declared_version, "wsrp-a-revision-nobody-shipped")
})
