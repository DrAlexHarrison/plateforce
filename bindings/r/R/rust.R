#' @useDynLib plateforce, .registration = TRUE
NULL

# The compiled entry points, one line each. Every answer is a JSON document and every
# document is turned into R data by the same compiled walk, so no file in this package
# reads JSON a second way.

decode <- function(document) .Call(wrap__pf_decode, document)

rust_version_json <- function() .Call(wrap__pf_version_json)

rust_registry_json <- function(root) .Call(wrap__pf_registry_json, root)

rust_registry_entry_json <- function(root, id) {
  .Call(wrap__pf_registry_entry_json, root, id)
}

rust_bindings_json <- function() .Call(wrap__pf_bindings_json)

rust_trial_from_force <- function(force_newtons, request_json) {
  .Call(wrap__pf_trial_from_force, force_newtons, request_json)
}

rust_trial_from_file <- function(request_json) {
  .Call(wrap__pf_trial_from_file, request_json)
}

rust_trial_report_json <- function(handle) .Call(wrap__pf_trial_report_json, handle)

rust_trial_force <- function(handle) .Call(wrap__pf_trial_force, handle)

rust_analyse_json <- function(handle, request_json) {
  .Call(wrap__pf_analyse_json, handle, request_json)
}

rust_spread_json <- function(handle, request_json) {
  .Call(wrap__pf_spread_json, handle, request_json)
}
