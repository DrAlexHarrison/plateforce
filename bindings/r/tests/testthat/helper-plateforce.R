# A partial match anywhere in this suite is a warning the suite can be made to fail on.
# The option is set here rather than globally: a package that changed a global option
# would change how every other package the user has loaded behaves.
options(warnPartialMatchDollar = TRUE)

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
