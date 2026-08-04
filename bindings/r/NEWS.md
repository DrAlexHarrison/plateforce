# plateforce 0.1.0

First release.

* A jump height, a takeoff velocity or a net impulse read in R carries the rule that
  produced it, the parameters that rule read, and whether each value was stated by the
  caller or supplied by the rule.
* Reading a shortened property name is an error rather than a neighbouring property's
  value, and coercing a measured value to a bare number asks for `@value` by name.
* A rule that declines raises a condition classed by its code, so `tryCatch()` can branch
  on one code or on every refusal, and every field is read by its whole name.
* The registry ships inside the package. `pf_registry()@census` counts each population
  apart, with each derived count beside the denominator it was taken over.
* A force file is read by the engine, with the delimiter and the force column stated by
  the caller and reported back along with two counts: the samples that matched the sentinel
  convention, and the samples that carried no number at all.
* The R surface binds the engine through `extendr`, chosen 2026-08-01. The boundary
  carries structured data as JSON and the force trace as a raw double vector, so one file
  names the binding framework.
