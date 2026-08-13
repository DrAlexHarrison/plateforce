# Saved plates from an R session.
#
# The store is the terminal's. A plate written by `plateforce plate save` is the plate
# `pf_plate` reaches, so what is asserted here is one answer rather than two that agree: the
# revision a stated plate hashes to is the revision a saved one hashes to, and both are the
# string a result attributes to the plate it ran under.

members <- function(...) {
  stated <- list(
    filter_at_capture = "none",
    tare_state = "tared_before_trial",
    plate_natural_frequency_hz = 400,
    floor_surface = "concrete",
    firmware_version = "2.1"
  )
  replacements <- list(...)
  for (member in names(replacements)) stated[[member]] <- replacements[[member]]
  do.call(pf_acquisition, stated)
}

# A folder each test owns, so nothing here writes into the machine's own settings.
scratch <- function() tempfile("plateforce-plates-")

test_that("a saved plate fills a complete block on a later run", {
  folder <- scratch()
  pf_plate_save("lab-kistler-1", members(), plates_folder = folder)

  trial <- pf_trial(rep(700, 1200),
    sample_rate_hz = 1200,
    plate = "lab-kistler-1", plates_folder = folder
  )

  expect_true(trial@acquisition_complete)
  expect_equal(trial@acquisition_missing, character(0))
})

test_that("a member stated beside a plate wins and the record says what it replaced", {
  folder <- scratch()
  pf_plate_save("lab-kistler-1", members(), plates_folder = folder)

  trial <- pf_trial(rep(700, 1200),
    sample_rate_hz = 1200,
    acquisition = pf_acquisition(firmware_version = "2.2"),
    plate = "lab-kistler-1", plates_folder = folder
  )
  analysed <- unwrap(decode(rust_analyse_json(
    trial@handle, registry_root(),
    analysis_request_of(
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    )
  )))

  expect_true(analysed[["acquisition_complete"]])
  expect_equal(analysed[["acquisition"]][["firmware_version"]], "2.2")
  expect_equal(analysed[["plate_profile"]][["name"]], "lab-kistler-1")
  expect_equal(
    analysed[["plate_profile"]][["superseded_members"]][["firmware_version"]],
    "2.1"
  )
})

test_that("a run that named no plate carries the block and nothing to attribute", {
  trial <- pf_trial(rep(700, 1200),
    sample_rate_hz = 1200,
    acquisition = members()
  )
  analysed <- unwrap(decode(rust_analyse_json(
    trial@handle, registry_root(),
    analysis_request_of(
      weighing = "bwepoch.fixed_window",
      onset = "onset.threshold.noise_relative",
      takeoff = "takeoff.threshold.absolute_force"
    )
  )))

  expect_true(analysed[["acquisition_complete"]])
  expect_equal(analysed[["acquisition"]][["firmware_version"]], "2.1")
  # Absent rather than null, the way every surface writes it: a run with no saved plate
  # behind it has nothing to attribute.
  expect_false("plate_profile" %in% names(analysed))
})

test_that("a plate stated from its members and one saved here carry one revision", {
  folder <- scratch()
  saved <- pf_plate_save("lab-kistler-1", members(), plates_folder = folder)
  stated <- pf_plate("lab-kistler-1", members())

  expect_equal(stated[["revision"]], saved[["revision"]])
  expect_null(stated[["path"]])
  expect_type(saved[["path"]], "character")
})

test_that("saving over a name hands back the revision it replaced and the members that moved", {
  folder <- scratch()
  first <- pf_plate_save("lab-kistler-1", members(), plates_folder = folder)
  second <- pf_plate_save("lab-kistler-1", members(firmware_version = "2.2"),
    plates_folder = folder
  )

  expect_false(identical(second[["revision"]], first[["revision"]]))
  expect_equal(second[["replaced_revision"]], first[["revision"]])
  expect_equal(second[["replaced_members"]][[1]][["member"]], "firmware_version")
  expect_equal(second[["replaced_members"]][[1]][["was"]], "2.1")
  expect_equal(second[["replaced_members"]][[1]][["now"]], "2.2")
})

test_that("a plate short of a member says which, and a run filled from it reports the gap", {
  folder <- scratch()
  saved <- pf_plate_save("lab-partial", pf_acquisition(tare_state = "tared"),
    plates_folder = folder
  )

  expect_false(saved[["acquisition_complete"]])
  expect_true("firmware_version" %in% unlist(saved[["acquisition_missing"]]))

  trial <- pf_trial(rep(700, 1200),
    sample_rate_hz = 1200,
    plate = "lab-partial", plates_folder = folder
  )
  expect_false(trial@acquisition_complete)
})

test_that("the plates this machine holds are named in order beside the folder holding them", {
  folder <- scratch()
  for (name in c("lab-b", "lab-a")) pf_plate_save(name, members(), plates_folder = folder)

  held <- pf_plates(plates_folder = folder)

  expect_equal(vapply(held[["plates"]], function(one) one[["plate"]], character(1)),
    c("lab-a", "lab-b")
  )
  expect_equal(held[["plates_folder"]], folder)
})

test_that("a forgotten plate is gone and the ones beside it are not", {
  folder <- scratch()
  pf_plate_save("lab-a", members(), plates_folder = folder)
  pf_plate_save("lab-b", members(), plates_folder = folder)

  pf_plate_forget("lab-a", plates_folder = folder)

  held <- pf_plates(plates_folder = folder)
  expect_equal(vapply(held[["plates"]], function(one) one[["plate"]], character(1)), "lab-b")
  expect_error(pf_plate("lab-a", plates_folder = folder), class = "plateforce_refusal")
})

test_that("a plate nobody saved is refused under a code rather than a sentence", {
  folder <- scratch()
  refused <- tryCatch(pf_plate("lab-kistler-9", plates_folder = folder),
    plateforce_refusal = function(condition) condition
  )
  expect_equal(refused$code, "file_not_read")
  expect_true(grepl("lab-kistler-9", conditionMessage(refused), fixed = TRUE))
})

test_that("a name that would reach another folder is refused before anything is written", {
  folder <- scratch()
  for (name in c("../secrets", "lab/1", "", "lab.1")) {
    expect_error(pf_plate(name, members()), class = "plateforce_refusal")
    expect_error(pf_plate_save(name, members(), plates_folder = folder),
      class = "plateforce_refusal"
    )
  }
  expect_equal(pf_plate("lab-kistler_1", members())[["plate"]], "lab-kistler_1")
})
