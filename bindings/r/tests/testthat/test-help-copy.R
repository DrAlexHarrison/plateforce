# A help page is a string a user reads, so CONVENTIONS.md section 5 reaches it.
#
# `scripts/check-framing-copy.sh` holds that rule and its population is `web/`, which is why
# three help pages said "this build" and nothing went red. Section 5's own scope is "the
# browser, the command line, an error, or a Python exception", so the gap was the checker's
# rather than the convention's.
#
# The banned list is read out of CONVENTIONS.md rather than written here, in both directions:
# a phrase enforced here that section 5 does not ban would be a rule nobody agreed, and a
# phrase section 5 bans that nothing here matches would be a ban with no enforcement.

conventions_lines <- function() {
  path <- testthat::test_path("..", "..", "..", "..", "CONVENTIONS.md")
  skip_if_not(file.exists(path), "CONVENTIONS.md is not beside this test")
  readLines(path, warn = FALSE)
}

# The quoted phrases in section 5's banned paragraph, which wraps across lines.
banned_phrases <- function(lines) {
  start <- grep("^Banned outright:", lines)
  if (!length(start)) return(character(0))
  blank <- grep("^\\s*$", lines)
  stop_at <- blank[blank > start[1]][1]
  block <- paste(lines[start[1]:(stop_at - 1)], collapse = " ")
  unlist(regmatches(block, gregexpr('"[^"]+"', block))) |>
    (\(quoted) gsub('^"|"$', "", quoted))()
}

# Every line of reader-facing help text this package writes, as (where, text).
roxygen_lines <- function() {
  root <- testthat::test_path("..", "..", "R")
  skip_if_not(dir.exists(root), "the sources are not beside this test")
  files <- list.files(root, pattern = "[.]R$", full.names = TRUE)
  out <- list()
  for (file in files) {
    lines <- readLines(file, warn = FALSE)
    hits <- grep("^\\s*#'", lines)
    for (number in hits) {
      out[[length(out) + 1]] <- list(
        where = paste0(basename(file), ":", number),
        text = sub("^\\s*#'\\s?", "", lines[number])
      )
    }
  }
  out
}

test_that("no help page describes the state of the software", {
  lines <- conventions_lines()
  banned <- banned_phrases(lines)
  expect_true(length(banned) > 0,
              info = "CONVENTIONS.md yielded no banned list, so this is checking itself")

  # Section 5's general clause, which is the authority for the phrase below that its
  # illustrative list does not spell out. Quoted from the file rather than written here, so a
  # rewrite of section 5 that drops the clause fails here rather than leaving this enforcing
  # a sentence the document has stopped saying.
  # Joined and whitespace-normalised: the clause wraps across two lines, and a line-by-line
  # search for it comes back empty against a document that says it.
  document <- paste(lines, collapse = " ")
  document <- gsub("\\s+", " ", document)
  expect_true(grepl("never describes the state of this software", document, fixed = TRUE),
              info = "section 5 no longer states the general rule this test enforces")

  # "in this build" is on section 5's list and "this build" is the form that got through, so
  # the shorter one is enforced under the general clause rather than the list.
  patterns <- unique(c(banned, "this build"))

  help <- roxygen_lines()
  expect_true(length(help) >= 100,
              info = paste("read", length(help), "help lines, too few to have read the package"))

  # The scan reaching a page whose words are known, located by anchor rather than copied, so
  # a glob narrowed to nothing cannot read as a package that stopped saying these things.
  expect_true(any(vapply(help, function(one) grepl("^registry[.]R:", one$where), logical(1))),
              info = "the scan did not reach registry.R, which is where three of these were")

  # The scan can see what it is looking for. A sample carrying each phrase, run through the
  # same matcher, so a matcher that reads nothing fails here rather than on the package.
  for (phrase in patterns) {
    expect_true(any(grepl(phrase, paste("a line that says", phrase), ignore.case = TRUE)),
                info = paste("the matcher cannot see", phrase, "in a line written to carry it"))
  }

  found <- character(0)
  for (one in help) {
    for (phrase in patterns) {
      if (grepl(phrase, one$text, ignore.case = TRUE)) {
        found <- c(found, paste0(one$where, ': "', one$text, '" says "', phrase, '"'))
      }
    }
  }
  expect_equal(found, character(0),
               info = paste(length(found), "of", length(help),
                            "help lines describe the state of the software"))
})
