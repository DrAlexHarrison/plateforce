# Submission comments

## Test environments

* R-devel r90394 (2026-08-12), Windows Server 2022 x64, GCC 14.3,
  official win-builder
* R-devel r90394 (2026-08-12), Ubuntu 22.04, Clang 15
* R 4.6.1, Ubuntu 24.04, GCC 13.3, official R-hub release container
* R 4.6.1, macOS Tahoe 26.6 arm64, Apple Clang 21 (`--no-manual` because
  the local Mac has no `pdflatex`)
* R 4.3.3, Linux Mint 22.3, GCC 13.3
* rustc and Cargo 1.82.0, the declared minimum, for Linux and a Windows target

## R CMD check results

The exact source tarball produced:

* zero errors, zero warnings and one NOTE on win-builder R-devel;
* zero errors, zero warnings and one NOTE on Ubuntu R-devel;
* `Status: OK` in the official R-hub release container; and
* zero errors, zero warnings and one NOTE on native arm64 macOS.

The NOTE in those R-devel and macOS checks is:

```
* checking CRAN incoming feasibility ... NOTE
Maintainer: 'Alex Harrison <alex@saturdaymorning.fit>'

New submission
```

This is the first submission of this package. PDF and HTML manuals passed on
R-devel Linux, win-builder, the R-hub release container and the local R 4.3.3
installation.

The older local R 4.3.3 check produced zero errors, zero warnings and four
NOTEs. One is `New submission`. The other three are answered below.

## Local R 4.3.3 NOTES

### Installed size

Current R-devel reports 7.9 MB installed, with 7.1 MB under `libs`, as INFO.
The older local R reports 8.0 MB as a NOTE. The installed `libs` file is the R
shared library `plateforce.so`, 7,393,456 bytes, not a static archive. Cargo
creates `libplateforce_r.a` during compilation, links it into `plateforce.so`,
then removes it. The installed package contains no `.a` or `.o` file.

The source tarball is 1,934,903 bytes and contains no binary code. Its
1,262,500-byte `vendor.tar.xz` contains source for 31 locked Rust crates.

This meets accepted CRAN practice:

* CRAN's Rust guidance recommends bundling Rust dependencies with
  `cargo vendor`, using xz compression and avoiding installation-time
  downloads: <https://cran.r-project.org/web/packages/using_rust.html>.
* CRAN policy prefers bundled third-party source, permits source tarballs up to
  10 MB where possible, and requires static libraries on Windows and macOS:
  <https://cran.r-project.org/web/packages/policies.html>.
* The CRAN-hosted `rextendr` guide documents the same build pattern: compile a
  Rust `staticlib`, then link it into the R shared object that R loads:
  <https://stat.ethz.ch/CRAN/web/packages/rextendr/vignettes/package.html>.

The package does not strip the shared object because CRAN policy says packages
must retain diagnostic information. The remaining installed-size question is
reviewer discretion, not a departure from the Rust packaging guidance.

### Current time

The local R 4.3.3 host could not verify the current time. The same tarball
passed future-timestamp checks on current R-devel for Linux and Windows, current
R release Linux, and macOS.

### Compilation flag

`-mno-omit-leaf-frame-pointer` is inherited from
`/usr/lib/R/etc/Makeconf` by the local R 4.3.3 installation; it is absent from
the package sources and Makevars.

## Rust

`SystemRequirements` declares `Cargo (Rust's package manager), rustc >= 1.82`.
The configure scripts find both programs on `PATH` or in `~/.cargo/bin`, print
their versions, and reject a toolchain older than the declared minimum.

Compilation uses `cargo build --offline --locked -j 2`, with `CARGO_HOME`
inside the package build directory. `inst/AUTHORS` records every bundled crate,
version, declared licence and declared authorship. Exact rustc and Cargo 1.82.0
build the package for Linux and cross-compile it for
`x86_64-pc-windows-gnu`.

## Tests and examples

The installed test suite passes 412 expectations with no failures or warnings.
Nineteen repository-context tests skip because their repository-only fixtures
or source trees are not shipped in the package. Tests make no Internet request,
write only under R temporary storage, restore options and do not depend on
elapsed-time thresholds.

All 18 runnable examples pass. The slowest took 0.275 seconds locally, and
R-devel ran all examples in 3 seconds. The vignette rebuild passes.

## Package name

CRAN contains `forceplate`, version 1.1-5, by Raphael Hartmann and coauthors.
That package segments raw force-plate files into trials and computes descriptive
statistics over time bins. This submission, `plateforce`, computes jump kinetics
and attaches the full analysis method to every result. It is not a fork,
replacement or continuation of `forceplate`.
