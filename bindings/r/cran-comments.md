# Submission comments

## Test environments

* Linux x86_64, R 4.3.3, rustc 1.97.1
* The declared toolchain floor, rustc 1.82, built from a clean tree in CI

## R CMD check results

Zero errors, zero warnings, four NOTEs.

```
* checking CRAN incoming feasibility ... NOTE
Maintainer: 'Alex Harrison <alex@saturdaymorning.fit>'
New submission
```

This is the first submission of this package.

```
* checking for future file timestamps ... NOTE
unable to verify current time
```

The check machine could not reach the time service it queries.

```
* checking installed package size ... NOTE
  installed size is  5.1Mb
  sub-directories of 1Mb or more:
    libs   4.3Mb
```

The shared object statically links the compiled analysis engine, which is what makes the
R surface produce the same numbers as the other surfaces rather than a second
implementation of them.

```
* checking compilation flags used ... NOTE
Compilation used the following non-portable flag(s):
  '-mno-omit-leaf-frame-pointer'
```

That flag comes from `CFLAGS` in this R installation's own `etc/Makeconf`, which the
distribution set when it built R. The package sets no compilation flags of its own.

## Rust

The package compiles a Rust component. `SystemRequirements` declares
`Cargo (Rust's package manager), rustc >= 1.82`, and `configure` checks for both on
`PATH` and in `~/.cargo/bin`, prints the versions it found before compilation begins, and
stops with the version found and the version needed when the toolchain is older.

Every third-party crate is bundled in `src/rust/vendor.tar.xz` and the build is offline.
The build runs `cargo build -j 2` and sets `CARGO_HOME` under the build directory, so it
writes nothing outside it. `inst/AUTHORS` lists every bundled crate with the licence and
authorship the crate declares, and the file is regenerated from the bundle at build time
rather than maintained by hand.

The declared floor, 1.82, sits below the oldest rustc on the CRAN check farm, which was
1.84.1 on the six macOS flavours when read on 2026-08-01. `tools/check-floor.sh` compares
the three numbers on every build and prints the age of that reading.

## Package name

There is an unrelated CRAN package named `forceplate`. This package is `plateforce`, and
the two are not related.
