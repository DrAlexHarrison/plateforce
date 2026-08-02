# Writes inst/AUTHORS from the vendored crates, because CRAN asks for the authorship and
# copyright of every Rust source the package carries.
#
# A crate declaring no licence stops this rather than producing a blank line. An
# unlicensed vendored crate is found either here or at review, and review costs a cycle.

here <- dirname(sub("^--file=", "", grep("^--file=", commandArgs(FALSE), value = TRUE)[1]))
if (is.na(here) || !nzchar(here)) here <- "tools"
package_root <- dirname(here)

vendor <- file.path(package_root, "src", "rust", "vendor")
bundle <- file.path(package_root, "src", "rust", "vendor.tar.xz")
extracted <- NULL

if (!dir.exists(vendor)) {
  if (!file.exists(bundle)) {
    stop("no vendored crates to read: run tools/vendor.sh first", call. = FALSE)
  }
  extracted <- tempfile("plateforce-vendor-")
  dir.create(extracted)
  utils::untar(bundle, exdir = extracted)
  vendor <- file.path(extracted, "vendor")
}

manifests <- list.files(vendor, pattern = "^Cargo[.]toml$", recursive = TRUE,
                        full.names = TRUE)
manifests <- manifests[dirname(dirname(manifests)) == vendor]

field <- function(lines, name) {
  # The package table ends at the next table header, so a `license` under `[dependencies]`
  # is not read as the crate's own.
  starts <- grep("^\\s*\\[", lines)
  package <- grep("^\\s*\\[package\\]", lines)
  if (!length(package)) return(NA_character_)
  after <- starts[starts > package[1]]
  block <- lines[package[1]:(if (length(after)) after[1] - 1L else length(lines))]
  hit <- grep(paste0("^\\s*", name, "\\s*="), block, value = TRUE)
  if (!length(hit)) return(NA_character_)
  value <- sub(paste0("^\\s*", name, "\\s*=\\s*"), "", hit[1])
  trimws(gsub('^\\[|\\]$|"', "", value))
}

rows <- lapply(manifests, function(path) {
  lines <- readLines(path, warn = FALSE)
  name <- field(lines, "name")
  licence <- field(lines, "license")
  if (is.na(licence) || !nzchar(licence)) {
    licence_file <- field(lines, "license-file")
    if (is.na(licence_file) || !nzchar(licence_file)) {
      stop(sprintf("%s declares no licence", name), call. = FALSE)
    }
    licence <- paste("see", licence_file)
  }
  authors <- field(lines, "authors")
  if (is.na(authors) || !nzchar(authors)) authors <- "not stated by the crate"
  data.frame(
    name = name,
    version = field(lines, "version"),
    licence = licence,
    authors = authors,
    stringsAsFactors = FALSE
  )
})

table <- do.call(rbind, rows)
table <- table[order(table$name), , drop = FALSE]

lines <- c(
  "Rust sources bundled with this package, one line per crate: name, version, the",
  "Licence the crate declares, and the authorship the crate declares.",
  "",
  sprintf("%s %s, %s, %s", table$name, table$version, table$licence, table$authors)
)

dir.create(file.path(package_root, "inst"), showWarnings = FALSE)
writeLines(lines, file.path(package_root, "inst", "AUTHORS"))

if (!is.null(extracted)) unlink(extracted, recursive = TRUE)
cat(sprintf("inst/AUTHORS: %d crates\n", nrow(table)))
