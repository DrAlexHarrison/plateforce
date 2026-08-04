# A partial match anywhere in this suite is a warning the suite can be made to fail on.
# The option is set here rather than globally: a package that changed a global option
# would change how every other package the user has loaded behaves.
options(warnPartialMatchDollar = TRUE)

# One link of a provenance chain, found by the rule's id rather than by where it sits.
# A chain gains a link whenever a rule runs earlier than the ones already in it, so a test
# that reads a position is asserting an ordering it did not mean to assert.
link_named <- function(chain, method_id) {
  for (link in chain) {
    if (identical(link@method_id, method_id)) return(link)
  }
  testthat::fail(paste(method_id, "is not in this chain"))
}

# One record and every record upstream of it, depth first.
#
# A test that walks `@depends_on` alone reads the rules one step under the root and stops. The
# operators sit under the landmark rule they compose onto, so a loop over one level asserts
# nothing about a third of the tree and shrinks silently as the tree gains depth.
every_step <- function(record) {
  c(list(record), unlist(lapply(record@depends_on, every_step), recursive = FALSE))
}

# One provenance chain as text, depth first, each step naming every value it read and where
# that value came from. Quantities and named choices together, in that order: they move the
# number equally and the record keeps them apart only because a fingerprint does.
#
# Written here and read by both the fixture writer and the test that holds a live chain to the
# fixture, so the two cannot render one record two ways and agree about it.
chain_lines <- function(record, depth = 0L) {
  bound <- rbind(record@parameters, record@choices)
  named <- if (nrow(bound)) {
    paste0(" ", paste(
      paste0(bound[["name"]], "=", bound[["value"]], "(", bound[["source"]], ")"),
      collapse = " "
    ))
  } else {
    ""
  }
  c(
    paste0(strrep("  ", depth), record@method_id, named),
    unlist(lapply(record@depends_on, chain_lines, depth = depth + 1L))
  )
}

fixture_lines <- function(name) {
  path <- testthat::test_path("fixtures", name)
  testthat::skip_if_not(file.exists(path), paste("no fixture at", path))
  readLines(path)
}

fixture_field <- function(name, field) {
  lines <- fixture_lines(name)
  hit <- lines[startsWith(lines, paste0(field, " "))]
  if (!length(hit)) testthat::skip(paste(field, "is not in", name))
  sub(paste0("^", field, " "), "", hit[1])
}

# The trace these fixtures were derived from lives in the repository rather than in the
# package, so the tests that recompute from it run where it is.
repository_trace <- function() {
  candidates <- c(
    file.path("..", "..", "..", "..", "crates", "plateforce-conformance", "fixtures"),
    file.path("..", "..", "..", "crates", "plateforce-conformance", "fixtures")
  )
  for (directory in candidates) {
    path <- file.path(testthat::test_path(directory), "subject01_trial1.force.txt")
    if (file.exists(path)) return(normalizePath(path))
  }
  NULL
}

quiet_trial <- function(samples = 1200, newtons = 700) {
  pf_trial(rep(newtons, samples), sample_rate_hz = 1200)
}
