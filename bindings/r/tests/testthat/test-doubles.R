# A number reaches R as text and what R holds afterwards is whatever the reader made of
# those digits. The writer is correct; a reader that is not correctly rounded loses the
# last bit on a few values in a hundred, and a value one bit out is a value a manuscript
# reports wrongly. Byte equality over the serialised document cannot see this, because
# the text is identical on both sides of the defect.
#
# Measured on this boundary before the reader was made correctly rounded: 18223 of 200000.

bits_of <- function(x) {
  bytes <- writeBin(as.double(x), raw(), size = 8, endian = "little")
  vapply(seq_along(x), function(i) {
    paste(rev(as.character(bytes[((i - 1) * 8 + 1):(i * 8)])), collapse = "")
  }, character(1))
}

test_that("a double crossing into R arrives with every bit it was sent with", {
  count <- 20000L
  probe <- decode(rust_double_probe_json(count))[["ok"]]
  values <- as.double(unlist(probe[["values"]]))
  declared <- unlist(probe[["bits"]])

  expect_identical(length(values), count)
  expect_identical(sum(bits_of(values) != declared), 0L)
})

test_that("the probe spans the range these quantities live in", {
  probe <- decode(rust_double_probe_json(2000L))[["ok"]]
  values <- as.double(unlist(probe[["values"]]))

  expect_lt(min(values), 0.01)
  expect_gt(max(values), 100)
})
