#!/usr/bin/env bash
# Builds a wheel from this checkout, installs it somewhere of its own, and names the
# interpreter carrying it.
#
# Both Python rows build what they ask, which is what scripts/capability-surfaces.txt
# requires of every row. Asking whichever `plateforce` happens to be importable reads
# whatever was last installed on the machine: a wheel built from another branch answers for
# that branch and reports a match while doing it. The R row met the same wall and answers it
# the same way, in scripts/capability-from-r.sh, and a shared library there made the parity
# gate certify a package built from a different branch.
#
# The environment is this repository's own rather than the operator's, for the same reason.
# Installing into whatever interpreter invoked this would replace a package somebody is
# working with, and would then answer for this checkout everywhere else on the machine.
#
# Everything this prints except the interpreter path goes to the other stream, because the
# caller reads this one.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
home="$root/target/python-surface"
environment="$home/venv"
wheels="$home/wheels"

if [[ ! -x "$environment/bin/python" ]]; then
    python3 -m venv "$environment" >&2
fi
python="$environment/bin/python"

# maturin from the path where the machine already has it, and into this environment when it
# does not. Named explicitly rather than left to a build frontend, because the frontend would
# fetch its own copy into a temporary directory on every run.
if command -v maturin >/dev/null 2>&1; then
    maturin=maturin
elif [[ -x "$environment/bin/maturin" ]]; then
    maturin="$environment/bin/maturin"
else
    "$python" -m pip install --quiet maturin >&2
    maturin="$environment/bin/maturin"
fi

# Removed rather than left to accumulate: a directory holding two wheels has no single answer
# to which one was just built, and the install below would pick by sort order.
rm -rf "$wheels"

# Destroy before producing, which is what the other three surface scripts already do:
# `r-surface.sh` clears its library and `build-web.sh` clears its output directory, so a
# failure in either leaves nothing and the consumer fails loudly. This script did not, and the
# environment is created only when missing and never cleared, so a failed `maturin build` exited
# here leaving the PREVIOUS run's compiled wheel installed. Anything that then imported
# `plateforce` from this environment answered for a tree that is not the one under test, and
# reported green. That is the stale-installed-package trap, which this project has now met nine
# times in four days, in the one lane that had no guard against it.
#
# Scoped to the package under test rather than to the whole environment, so `pytest` and its
# dependencies survive and a run costs no more than it did.
"$python" -m pip uninstall --quiet --yes plateforce >&2 || true

"$maturin" build \
    --manifest-path "$root/crates/plateforce-python/Cargo.toml" \
    --interpreter "$python" \
    --out "$wheels" >&2

built=("$wheels"/*.whl)
if [[ ${#built[@]} -ne 1 ]]; then
    echo "the build wrote ${#built[@]} wheels and this row asks one question of one wheel" >&2
    exit 1
fi

# `--no-index` because every dependency this wheel has is compiled into it, so a run that
# reached an index would be reaching for something that does not exist, and would fail slowly
# on a machine with no network rather than immediately.
"$python" -m pip install --quiet --force-reinstall --no-deps --no-index "${built[0]}" >&2

echo "$python"
