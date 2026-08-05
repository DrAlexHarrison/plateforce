#' @include acquisition.R rust.R refusal.R
NULL

# The store is the engine's, the same one `plateforce plate save` writes, so a plate recorded
# at the terminal is the plate an R session on that machine names. Nothing here decides where
# a file lives or what a name may be.

#' Record a plate's settings
#'
#' Answers five questions once instead of at every analysis. A run told about a saved plate
#' fills its acquisition block from it and records which plate and which revision of it the
#' answers came off.
#'
#' Saving over a name changes what later runs are told. The revision that was there before
#' comes back in `replaced_revision`, and each member whose answer moved in
#' `replaced_members`, because a result recorded earlier rests on answers this machine no
#' longer holds.
#'
#' @param name What to call this plate. Letters, digits, `-` and `_`.
#' @param acquisition What the plate and its settings were, from [pf_acquisition()].
#' @param plates_folder Keep saved plates here rather than beside this machine's other
#'   settings. A plate saved under a named folder travels with a dataset; one saved without
#'   travels with the person.
#' @return A list carrying the plate, its revision, where it is filed, the block it holds and
#'   the members still to answer, with `replaced_revision` and `replaced_members` where a
#'   plate of that name was already saved.
#' @export
#' @examples
#' folder <- tempfile("plates")
#' saved <- pf_plate_save(
#'   "lab-1",
#'   pf_acquisition(tare_state = "tared_before_trial"),
#'   plates_folder = folder
#' )
#' saved[["revision"]]
#' saved[["acquisition_missing"]]
pf_plate_save <- function(name, acquisition = NULL, plates_folder = NULL) {
  unwrap(decode(rust_plate_save_json(request_of(
    name = name,
    acquisition = acquisition,
    plates_folder = plates_folder
  ))))
}

#' One plate, saved here or stated from its members
#'
#' Naming `acquisition` states a plate from the members the caller holds, with no file behind
#' it, which is how a plate travels between machines. Leaving it out reads the one this
#' machine has saved under that name.
#'
#' The revision is taken from the members either way, so a plate stated from a colleague's
#' answers and the same plate saved here carry one revision and one attribution.
#'
#' @param name The plate to read, or to call the stated one.
#' @param acquisition The members it holds, from [pf_acquisition()]. Absent reads a saved one.
#' @inheritParams pf_plate_save
#' @return A list carrying the plate, its revision, where it is filed when it is, the block it
#'   holds and the members still to answer.
#' @export
#' @examples
#' folder <- tempfile("plates")
#' pf_plate_save("lab-1", pf_acquisition(floor_surface = "concrete"), plates_folder = folder)
#' pf_plate("lab-1", plates_folder = folder)[["acquisition"]][["floor_surface"]]
#'
#' # The same plate, stated rather than read.
#' pf_plate("lab-1", pf_acquisition(floor_surface = "concrete"))[["revision"]]
pf_plate <- function(name, acquisition = NULL, plates_folder = NULL) {
  if (is.null(acquisition)) {
    return(unwrap(decode(rust_plate_json(request_of(
      name = name,
      plates_folder = plates_folder
    )))))
  }
  unwrap(decode(rust_plate_stated_json(request_of(
    name = name,
    acquisition = acquisition
  ))))
}

#' Every plate this machine holds
#'
#' @inheritParams pf_plate_save
#' @return A list carrying `plates_folder` and one entry per saved plate.
#' @export
#' @examples
#' folder <- tempfile("plates")
#' pf_plate_save("lab-1", pf_acquisition(tare_state = "tared"), plates_folder = folder)
#' length(pf_plates(plates_folder = folder)[["plates"]])
pf_plates <- function(plates_folder = NULL) {
  unwrap(decode(rust_plates_json(request_of(plates_folder = plates_folder))))
}

#' Remove a saved plate
#'
#' Results already recorded against it carry its members and are unchanged.
#'
#' @param name The plate to remove.
#' @inheritParams pf_plate_save
#' @return A list naming the plate and the file that was removed.
#' @export
#' @examples
#' folder <- tempfile("plates")
#' pf_plate_save("lab-1", pf_acquisition(tare_state = "tared"), plates_folder = folder)
#' pf_plate_forget("lab-1", plates_folder = folder)[["plate"]]
pf_plate_forget <- function(name, plates_folder = NULL) {
  unwrap(decode(rust_plate_forget_json(request_of(
    name = name,
    plates_folder = plates_folder
  ))))
}

# The plate a trial request states: the name a reader recognises and the members it holds.
# The revision is taken from the members by the engine, never written here, because two
# spellings of it would let one plate look like two.
#
# A saved plate is read here and sent as its members rather than as a name, so a trial built
# from a plate this machine holds and one built from a plate a colleague sent take the same
# path and produce the same attribution.
stated_plate_of <- function(plate, plates_folder = NULL) {
  if (is.null(plate)) {
    return(NULL)
  }
  if (is.character(plate)) {
    if (length(plate) != 1L) {
      refuse_here(
        "value_not_accepted",
        "a trial is filled from one saved plate",
        parameter = "plate",
        value = length(plate)
      )
    }
    plate <- pf_plate(plate, plates_folder = plates_folder)
  }
  members <- plate[["acquisition"]]
  if (!is.null(members[["plate_natural_frequency_hz"]])) {
    members[["plate_natural_frequency_hz"]] <- as.numeric(
      members[["plate_natural_frequency_hz"]]
    )
  }
  list(name = plate[["plate"]], members = members)
}
