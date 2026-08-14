# Submission comments

## Test environments

* R-devel r90394 (2026-08-12), Windows Server 2022 x64, GCC 14.3,
  win-builder
* R-devel r90394 (2026-08-12), Ubuntu 22.04 x64, Clang 15
* R 4.6.1 Patched r90311 (2026-07-27), macOS Tahoe 26.6 arm64,
  Apple Clang 17, official macOS builder
* R 4.6.1 (2026-06-24), Ubuntu 24.04 x64, GCC 13.3, official R-hub
  release container

## R CMD check results

There were zero errors and zero warnings on every platform. The macOS builder
and R-hub release checks returned `Status: OK`. Each R-devel check returned one
NOTE:

```
* checking CRAN incoming feasibility ... NOTE
Maintainer: 'Alex Harrison <alex@saturdaymorning.fit>'

New submission
```

This is the package's first CRAN submission, so the `New submission` NOTE
cannot be removed before submission.

Tests, examples, vignette rebuilding and the PDF manual passed. Installation
uses the Rust sources bundled in the source package and makes no network
request.
