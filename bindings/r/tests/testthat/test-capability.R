# The manifest is produced by the engine, so comparing it against the engine proves nothing
# about R: a forwarded document agrees with itself whatever this package can actually do.
# What is about R is that every operation it names resolves to something a caller can call.
#
# The mapping is data rather than a switch, and it is the one place the per-surface spelling
# of an operation lives.

operation_exports <- data.frame(
  operation = c("analyse", "capability", "parse_force_file", "registry_census",
                "registry_show", "spread", "version"),
  export = c("analyse_countermovement_jump", "capability_json", "pf_read_force_file",
             "pf_registry", "pf_entry", "pf_spread", "pf_version"),
  stringsAsFactors = FALSE
)

manifest <- function() {
  decode(capability_json())[["ok"]]
}

test_that("every operation this package names resolves to one of its exports", {
  named <- as.character(unlist(manifest()[["operations"]]))
  exported <- getNamespaceExports("plateforce")

  expect_true(length(named) > 0)
  expect_setequal(named, operation_exports[["operation"]])

  unresolved <- character(0)
  for (index in seq_along(named)) {
    row <- operation_exports[operation_exports[["operation"]] == named[index], , drop = FALSE]
    if (!nrow(row) || !row[["export"]] %in% exported) {
      unresolved <- c(unresolved, named[index])
    }
  }

  expect_identical(unresolved, character(0),
                   info = sprintf("%d of %d operations resolved",
                                  length(named) - length(unresolved), length(named)))
})

test_that("a container format is claimed only where a writer exists", {
  formats <- manifest()[["output_formats"]]
  writers <- grep("^pf_write_", getNamespaceExports("plateforce"), value = TRUE)

  expect_identical(length(formats), length(writers))
})

test_that("every refusal code the manifest names carries a shell exit status", {
  codes <- manifest()[["refusal_codes"]]

  expect_true(length(codes) > 0)
  for (record in codes) {
    expect_true(nzchar(record[["code"]]))
    # EX_USAGE, EX_DATAERR, EX_NOINPUT, EX_CONFIG. A file that cannot be opened takes its
    # own status because a workflow manager retries bad data and stops on a missing path.
    expect_true(record[["exit_code"]] %in% c(64L, 65L, 66L, 78L))
    expect_identical(record[["code"]], tolower(record[["code"]]))
  }
})

# Conditions this surface can be in that the engine has no code for: reading a property by
# a shortened name, asking for a quantity the analysis did not report, handing a trace that
# is not a double vector. Each is about how R is being used rather than about a rule, so the
# engine has nothing to say about it and the manifest cannot list it.
#
# The list is here so a code that drifts out of the engine's vocabulary has to be added
# deliberately rather than arriving unnoticed.
SURFACE_ONLY_CODES <- c(
  "field_not_named_in_full",
  "force_not_double",
  "parameter_source_unrecorded",
  "provenance_dropped",
  "quantity_not_reported",
  "registry_not_found",
  "slot_has_no_rules",
  "unknown_field"
)

# Every code the R sources raise, read off those sources rather than off whichever ones a
# test happens to reach. Two shapes: the code as the first argument to this package's own
# raiser, and the code as a named field on a condition built where it is raised. Both
# tolerate a line wrap, because a pattern anchored to one line reads two of these as nine.
codes_raised_in_the_r_sources <- function(root) {
  text <- paste(
    unlist(lapply(
      list.files(root, pattern = "[.]R$", full.names = TRUE),
      function(file) readLines(file, warn = FALSE)
    )),
    collapse = "\n"
  )
  matched <- c(
    regmatches(text, gregexpr('refuse_here\\(\\s*"[a-z_]+"', text))[[1]],
    regmatches(text, gregexpr('\\bcode = "[a-z_]+"', text))[[1]]
  )
  sort(unique(gsub('.*"([a-z_]+)".*', "\\1", matched)))
}

raised <- function(expression) {
  tryCatch(expression, plateforce_refusal = identity)
}

test_that("a refusal about a rule is spelled the way the engine spells it", {
  codes <- vapply(manifest()[["refusal_codes"]], function(r) r[["code"]], character(1))

  for (condition in list(
    raised(pf_trial(rep(700, 12), sample_rate_hz = 12, sentinel_convention = "nine_thousand")),
    raised(pf_trial(rep(700, 12))),
    raised(pf_read_force_file(tempfile(), sample_rate_hz = 12))
  )) {
    expect_s3_class(condition, "plateforce_refusal")
    expect_true(condition[["code"]] %in% codes,
                info = paste(condition[["code"]], "is not one of the manifest's",
                             length(codes), "codes"))
    expect_true(paste0("plateforce_", condition[["code"]]) %in% class(condition))
  }
})

test_that("a refusal about how this surface is being used is declared as such", {
  codes <- vapply(manifest()[["refusal_codes"]], function(r) r[["code"]], character(1))

  for (condition in list(
    raised(pf_trial(rep(700L, 12), sample_rate_hz = 12)),
    raised(pf_spread(quiet_trial(), quantity = "system_weight_newtons", slot = "landing",
                     weighing = "bwepoch.fixed_window",
                     onset = "onset.threshold.noise_relative",
                     takeoff = "takeoff.threshold.absolute_force"))
  )) {
    expect_true(condition[["code"]] %in% SURFACE_ONLY_CODES,
                info = paste(condition[["code"]], "is neither a manifest code nor declared here"))
    expect_false(condition[["code"]] %in% codes)
  }
})

# The list above says it exists so a code drifting out of the engine's vocabulary has to be
# added deliberately rather than arriving unnoticed. Two arrived unnoticed, because the only
# assertion over it was that the codes a test happens to raise are members of it, and a list
# is never short of the codes nobody exercised. The set is derived from the sources here, so
# a code raised and not declared is a failure rather than a member nobody looked for.
test_that("every code the R sources raise is either the engine's or declared here", {
  # An installed package has an `R/` directory too, holding the lazy-load database rather
  # than any source file. Falling back to it made the directory test pass with nothing to
  # read, so under `R CMD check` this scanned an empty set, the control below read it as a
  # package that raises no codes, and every declared code then looked stale. What decides
  # whether this test can run is the presence of the sources, so that is what is asked.
  root <- testthat::test_path("..", "..", "R")
  sources <- if (dir.exists(root)) list.files(root, pattern = "\\.R$") else character(0)
  skip_if_not(length(sources) > 0, "the sources are not beside this test")

  raised_codes <- codes_raised_in_the_r_sources(root)
  # The control. A pattern that has stopped matching reads as a package that raises nothing,
  # which is indistinguishable from a clean one.
  expect_true(length(raised_codes) > 0)

  published <- vapply(manifest()[["refusal_codes"]], function(r) r[["code"]], character(1))
  undeclared <- setdiff(raised_codes, c(published, SURFACE_ONLY_CODES))
  expect_identical(
    undeclared, character(0),
    info = sprintf("%d of %d codes raised in R/ are neither published nor declared: %s",
                   length(undeclared), length(raised_codes),
                   paste(undeclared, collapse = ", "))
  )

  # And the other direction, so the list cannot keep a name nothing raises any more.
  stale <- setdiff(SURFACE_ONLY_CODES, raised_codes)
  expect_identical(
    stale, character(0),
    info = paste("declared and raised nowhere:", paste(stale, collapse = ", "))
  )
})
