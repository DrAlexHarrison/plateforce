#!/usr/bin/env bash
#
# One version, from the manifests to every place a reader can get it.
#
#   ./scripts/ship.sh 0.1.2              the whole route
#   ./scripts/ship.sh 0.1.2 --dry-run    every check, no push, no tag, no publish
#   ./scripts/ship.sh --resume           carry on from wherever the last run stopped
#
# The route is: write the version into every home, regenerate what is derived from it,
# hold the tree to its own gates, get all six workflows green on ONE commit, tag, let the
# tag build and sign five platforms, verify the artefacts by running them, publish, then
# carry the new version to Homebrew and to the folder handed to someone in person.
#
# Every step states what it read as well as whether it passed, because a step that only
# prints "ok" cannot tell a working check from one that looked at nothing. Each step also
# writes its name into the state file on completion, so --resume starts at the first step
# that has not finished rather than at the beginning.
#
# The two credentials this needs are read the same way the rest of the repository reads
# them: GH_TOKEN if it is already set, otherwise a local secret store if it is present,
# otherwise whatever `gh auth` holds. The tap is pushed over ssh so it needs no token at
# all.

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

state_file=".git/ship-state"
dry_run="no"
version=""
resume="no"

for argument in "$@"; do
  case "$argument" in
    --dry-run) dry_run="yes" ;;
    --resume) resume="yes" ;;
    -*) echo "$0: $argument is not an option this takes" >&2; exit 2 ;;
    *) version="$argument" ;;
  esac
done

if [ "$resume" = "yes" ] && [ -z "$version" ]; then
  version="$(sed -n 's/^version=//p' "$state_file" 2>/dev/null || true)"
  [ -n "$version" ] || { echo "$0: nothing to resume, no state file" >&2; exit 2; }
fi

if [ -z "$version" ]; then
  echo "usage: $0 <version> [--dry-run] | $0 --resume" >&2
  exit 2
fi

case "$version" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "$0: '$version' is not a three part version" >&2; exit 2 ;;
esac

tag="v${version}"

say() { printf '\n=== %s\n' "$*"; }
note() { printf '    %s\n' "$*"; }

# A step is skipped only when the state file says it finished for THIS version, so a
# resume after a version change starts over rather than trusting the last one's work.
done_already() {
  [ "$resume" = "yes" ] || return 1
  grep -qx "$1" "$state_file" 2>/dev/null
}

finished() {
  [ "$dry_run" = "yes" ] && return 0
  mkdir -p "$(dirname "$state_file")"
  grep -qx "version=$version" "$state_file" 2>/dev/null || printf 'version=%s\n' "$version" > "$state_file"
  printf '%s\n' "$1" >> "$state_file"
}

token() {
  if [ -n "${GH_TOKEN:-}" ]; then printf '%s' "$GH_TOKEN"; return; fi
  local store="$HOME/.claude/scripts/gcp-secret.sh"
  if [ -x "$store" ]; then "$store" plateforce-release-pat 2>/dev/null && return; fi
  printf ''
}

gh_() {
  local held
  held="$(token)"
  if [ -n "$held" ]; then GH_TOKEN="$held" gh "$@"; else gh "$@"; fi
}

# ---------------------------------------------------------------- preflight

say "Preflight"

if [ "$(git rev-parse --abbrev-ref HEAD)" != "main" ]; then
  echo "not on main, and a release is cut from main" >&2
  exit 1
fi

git fetch --quiet origin
if [ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]; then
  echo "main and origin/main differ, so the tag would not name what CI tested" >&2
  exit 1
fi

# An existing tag stops a fresh cut, because a published version is never re-cut. It must
# NOT stop a resume that has already tagged: everything after tagging is verifying and
# publishing what the tag built, and refusing there strands a release between its tag and
# its assets, which is the worst place to leave one.
if git rev-parse "$tag" >/dev/null 2>&1 && ! done_already "tag"; then
  echo "$tag already exists locally. A published version is never re-cut." >&2
  echo "To carry on with a release that already tagged, pass --resume." >&2
  exit 1
fi

if ! grep -aq "^## ${version}," CHANGELOG.md; then
  echo "CHANGELOG.md has no '## ${version},' section." >&2
  echo "What changed for a user is a judgment, so this script will not invent one." >&2
  exit 1
fi
note "changelog carries a ${version} section"
note "gh authenticates as $(gh_ api /user --jq .login 2>/dev/null || echo 'unknown')"
[ "$dry_run" = "yes" ] && note "DRY RUN: nothing will be pushed, tagged or published"

# ---------------------------------------------------------------- the version

if ! done_already "bump"; then
  say "Writing ${version} into every home"

  python3 - "$version" <<'PYTHON'
import re, sys, json, pathlib
version = sys.argv[1]

manifests = [
    ("Cargo.toml", r'(?m)^(version = ")[^"]+(")'),
    ("src-tauri/Cargo.toml", r'(?m)^(version = ")[^"]+(")'),
    ("bindings/r/src/rust/Cargo.toml", r'(?m)^(version = ")[^"]+(")'),
]
for path, pattern in manifests:
    text = pathlib.Path(path).read_text()
    written, count = re.subn(pattern, lambda m: m.group(1) + version + m.group(2), text, count=1)
    if count != 1:
        raise SystemExit(f"{path}: found {count} version lines, expected exactly 1")
    pathlib.Path(path).write_text(written)
    print(f"    {path}")

# Cargo's workspace dependency table names the version of every member it path-depends on,
# so a bump that misses it leaves the workspace describing two versions of itself.
root = pathlib.Path("Cargo.toml")
text = root.read_text()
written, count = re.subn(
    r'(\{ path = "[^"]+", version = ")[^"]+(")',
    lambda m: m.group(1) + version + m.group(2),
    text,
)
root.write_text(written)
print(f"    Cargo.toml, {count} path dependencies")

description = pathlib.Path("bindings/r/DESCRIPTION")
text = description.read_text()
written, count = re.subn(r"(?m)^(Version: ).*$", r"\g<1>" + version, text, count=1)
if count != 1:
    raise SystemExit(f"{description}: found {count} Version lines, expected exactly 1")
description.write_text(written)
print(f"    {description}")
PYTHON

  # The lockfiles are derived, so they are regenerated rather than edited. A hand-edited
  # lock is one more place the version can be wrong.
  cargo metadata --format-version 1 --no-deps >/dev/null

  # The R package carries its own copy of the engine, because the three crates are not on
  # crates.io and cargo will not vendor a path dependency. That copy is an untracked build
  # artefact regenerated in CI, but ITS LOCKFILE IS TRACKED, so a bump that does not
  # re-sync and relock leaves the lock naming the old version. CI vendors with --locked,
  # which then refuses to update it, and both the R workflow and the parity workflow go red
  # on a message about lockfiles that says nothing about versions. No `|| true` here: this
  # step failing silently is what produced that red.
  #
  # Only the four plateforce entries are rewritten. Re-resolving the whole lock instead
  # picks third-party crates published since, and the package declares a cargo floor that
  # a newer crate's manifest cannot even be parsed by: `feature edition2024 is required`
  # against cargo 1.82. The floor job then fails on a crate nobody here chose. So the
  # third-party pins are left exactly as they were.
  sh bindings/r/tools/sync-engine.sh >/dev/null
  python3 - "$version" <<'PYTHON'
import re, sys, pathlib
version = sys.argv[1]
path = pathlib.Path("bindings/r/src/rust/Cargo.lock")
text = path.read_text()
for name in ("plateforce-analysis", "plateforce-core", "plateforce-registry", "plateforce-r"):
    text, count = re.subn(
        rf'(name = "{re.escape(name)}"\n)version = "[^"]+"',
        lambda m: m.group(1) + f'version = "{version}"',
        text,
    )
    if count != 1:
        raise SystemExit(f"the R lockfile holds {count} entries for {name}, expected 1")
path.write_text(text)
print(f"    R lockfile, 4 engine entries at {version}")
PYTHON
  locked_engine="$(grep -a -A1 'name = "plateforce-core"' bindings/r/src/rust/Cargo.lock | sed -n 's/^version = "\(.*\)"/\1/p')"
  [ "$locked_engine" = "$version" ] \
    || { echo "the R lockfile pins the engine at ${locked_engine}, not ${version}" >&2; exit 1; }
  note "lockfiles current, R engine locked at ${locked_engine}"

  python3 scripts/verify-version-homes.py
  finished "bump"
fi

# ---------------------------------------------------------------- derived files

if ! done_already "derived"; then
  say "Regenerating what the version and the tree derive"

  cargo build -q -p plateforce-cli
  note "cli built, reports $(./target/debug/plateforce version)"

  python3 scripts/write-dependency-licences.py
  note "NOTICE rewritten"

  # The manifest carries the version once per surface, and regenerating it properly means
  # building all four, which is what parity.yml does on every commit. So the version is
  # written here and the rest of the document is left to that workflow, which is one of the
  # six that has to be green before this script will tag anything.
  python3 - "$version" <<'PYTHON'
import re, sys, pathlib
version = sys.argv[1]
path = pathlib.Path("CAPABILITY.json")
text = path.read_text()
written, count = re.subn(r'("plateforce_version": ")[^"]+(")', r"\g<1>" + version + r"\g<2>", text)
if count == 0:
    raise SystemExit("CAPABILITY.json names no plateforce_version, so nothing was written")
path.write_text(written)
print(f"    CAPABILITY.json, {count} surfaces")
PYTHON

  python3 scripts/check-changelog-identifiers.py
  finished "derived"
fi

# ---------------------------------------------------------------- the guides

if ! done_already "guides"; then
  say "Building the browser bundle and the guides"

  # A capture is a claim about web/pkg/, never about the source tree, so the bundle is
  # built before anything looks at a page.
  PATH="$HOME/.cargo/bin:$PATH" bash scripts/build-web.sh
  note "web bundle built"

  node docs/quickstart/capture.mjs 9731
  guides_markdown="$(mktemp -d)"
  python3 docs/quickstart/build.py --markdown-into "$guides_markdown"
  python3 docs/quickstart/check-guide-commands.py
  note "guides built, markdown in $guides_markdown"
  printf 'guides_markdown=%s\n' "$guides_markdown" >> "$state_file"
  finished "guides"
fi

guides_markdown="$(sed -n 's/^guides_markdown=//p' "$state_file" 2>/dev/null | tail -1)"

# ---------------------------------------------------------------- the commit

if ! done_already "commit"; then
  say "Committing the version"

  if [ -n "$(git status --porcelain)" ]; then
    git add -A
    if [ "$dry_run" = "yes" ]; then
      note "DRY RUN: would commit $(git diff --cached --name-only | wc -l | tr -d ' ') files"
      git reset --quiet
    else
      git commit -q -m "Version ${version}, in every manifest and everything derived from one"
      git push origin main
      # Piped push output reports the pipe, so the landing is read from the remote.
      [ "$(git rev-parse HEAD)" = "$(git ls-remote origin -h refs/heads/main | cut -f1)" ] \
        || { echo "the push did not land" >&2; exit 1; }
      note "pushed $(git rev-parse --short HEAD)"
    fi
  else
    note "nothing to commit, the tree already carries ${version}"
  fi
  finished "commit"
fi

sha="$(git rev-parse HEAD)"

# ---------------------------------------------------------------- six green

if ! done_already "green"; then
  say "Six workflows green on ${sha:0:7}"

  # Two of the six carry path filters, so a commit touching neither never runs them and
  # main reads green while they are red. A tag evaluates no path filter, so it would run
  # them for the first time against a commit nobody tested. Both are dispatched here.
  if [ "$dry_run" = "no" ]; then
    # Every path-filtered workflow is dispatched onto this exact commit. A tag evaluates no
    # path filter, so any of these that a commit did not touch would otherwise run for the
    # first time against the tag, which is the one moment nobody can fix it before it
    # publishes. The three desktop ones are here because they were NOT, and the v0.1.2 tag
    # failed inside one of their gates rather than in anything main had ever run.
    for workflow in python-wheels.yml r-package.yml registry-ids.yml parity.yml \
                    desktop-linux.yml desktop-macos.yml desktop-windows.yml; do
      gh_ workflow run "$workflow" --repo DrAlexHarrison/plateforce --ref main >/dev/null 2>&1 || true
    done
    note "dispatched the filtered workflows onto this commit"
    sleep 10
  fi

  while :; do
    states="$(gh_ run list --repo DrAlexHarrison/plateforce --limit 20 \
      --json workflowName,headSha,status,conclusion \
      --template '{{range .}}{{slice .headSha 0 7}} {{.status}} {{.conclusion}} {{.workflowName}}{{"\n"}}{{end}}' \
      2>/dev/null | grep -a "^${sha:0:7}" || true)"
    total="$(printf '%s' "$states" | grep -ac . || true)"
    complete="$(printf '%s' "$states" | grep -ac "completed" || true)"
    failed="$(printf '%s' "$states" | grep -a "completed" | grep -acv "success" || true)"

    if [ "$failed" -gt 0 ]; then
      printf '%s\n' "$states" | grep -a "completed" | grep -av "success"
      echo "a workflow is red on this commit, and a tag would publish it" >&2
      exit 1
    fi
    note "${complete} of ${total} complete, 0 red"
    [ "$total" -ge 6 ] && [ "$complete" -ge "$total" ] && break
    [ "$dry_run" = "yes" ] && { note "DRY RUN: not waiting"; break; }
    sleep 45
  done
  finished "green"
fi

# ---------------------------------------------------------------- the tag

if ! done_already "tag"; then
  say "Tagging ${tag}"

  identifiers="$(./target/debug/plateforce analyse \
    crates/plateforce-conformance/fixtures/subject01_trial1.force.txt \
    --column 0 --sentinel none --sample-rate-hz 1200 --preset sams --format json 2>/dev/null \
    | python3 -c 'import json,sys; d=json.load(sys.stdin)["ok"]; print(d["registry_declared_version"], d["registry_digest"])')"
  revision="${identifiers%% *}"
  digest="${identifiers##* }"

  message="$(mktemp)"
  {
    printf 'plateforce %s\n\n' "$version"
    printf 'Force-plate analysis where every result carries the method that produced it.\n\n'
    printf 'plateforce version                %s\n' "$version"
    printf 'method registry revision          %s\n' "$revision"
    printf 'method registry digest            %s\n\n' "$digest"
    printf 'Record those three beside any number you report. Installation routes are in\n'
    printf 'docs/install.md, and the quick start guides are attached to this release.\n'
  } > "$message"

  if [ "$dry_run" = "yes" ]; then
    note "DRY RUN: would tag ${tag} at ${sha:0:7} carrying ${digest}"
  else
    git tag -a "$tag" -F "$message"
    git push origin "$tag"
    git ls-remote --tags origin "$tag" | grep -aq . || { echo "the tag did not land" >&2; exit 1; }
    note "tagged ${tag}, ${digest}"
  fi
  rm -f "$message"
  finished "tag"
fi

# ---------------------------------------------------------------- the build

if ! done_already "built"; then
  say "Waiting for the tag to build five platforms"
  if [ "$dry_run" = "yes" ]; then
    note "DRY RUN: not waiting for a build that was never triggered"
  else
    while :; do
      state="$(gh_ run list --repo DrAlexHarrison/plateforce --limit 10 \
        --json workflowName,headBranch,status,conclusion \
        --template '{{range .}}{{.headBranch}} {{.workflowName}} {{.status}} {{.conclusion}}{{"\n"}}{{end}}' \
        2>/dev/null | grep -a "^${tag} release" || true)"
      case "$state" in
        *"completed success"*) note "release route green"; break ;;
        *"completed"*) printf '%s\n' "$state"; echo "the release route failed" >&2; exit 1 ;;
        *) note "building"; sleep 60 ;;
      esac
    done
  fi
  finished "built"
fi

# ---------------------------------------------------------------- verify by use

if ! done_already "verified"; then
  say "Verifying the artefacts by running them"

  if [ "$dry_run" = "yes" ]; then
    note "DRY RUN: no release exists to verify"
  else
    scratch="$(mktemp -d)"
    case "$(uname -s)" in
      Darwin) binary="plateforce-universal-macos" ;;
      Linux)  binary="plateforce-$(uname -m)-linux-static" ;;
      *)      binary="" ;;
    esac

    if [ -n "$binary" ]; then
      gh_ release download "$tag" --repo DrAlexHarrison/plateforce \
        --pattern "$binary" --dir "$scratch"
      chmod +x "$scratch/$binary"

      # Run it from a directory holding nothing, which is what a reader who downloaded one
      # file has, and compare its whole answer against this checkout's build rather than
      # comparing a version string that a broken binary would still print.
      reported="$("$scratch/$binary" version)"
      [ "$reported" = "plateforce ${version}" ] \
        || { echo "the published binary reports '$reported'" >&2; exit 1; }
      note "published binary reports ${reported}"

      fixture="$repository_root/crates/plateforce-conformance/fixtures/subject01_trial1.force.txt"
      arguments="--column 0 --sentinel none --sample-rate-hz 1200 --preset sams --format json"
      ( cd "$scratch" && ./"$binary" analyse "$fixture" $arguments > published.json )
      ./target/debug/plateforce analyse "$fixture" $arguments > "$scratch/local.json"
      python3 - "$scratch/published.json" "$scratch/local.json" <<'PYTHON'
import json, sys
published = json.load(open(sys.argv[1]))["ok"]
local = json.load(open(sys.argv[2]))["ok"]
published.pop("source_path", None)
local.pop("source_path", None)
if published != local:
    raise SystemExit("the published binary and this checkout disagree about the same trial")
metrics = {m["key"]: m.get("value") for m in published["metrics"]}
print(f"    {len(metrics)} of {len(published['metrics'])} metrics identical, "
      f"digest {published['registry_digest']}")
PYTHON

      if [ "$(uname -s)" = "Darwin" ]; then
        bash scripts/verify-macos-artefact.sh "$scratch/$binary" --executable >/dev/null
        note "gatekeeper accepts it as notarised software"
      fi
    fi
    rm -rf "$scratch"
  fi
  finished "verified"
fi

# ---------------------------------------------------------------- guides and publish

if ! done_already "published"; then
  say "Attaching the guides and publishing"

  if [ "$dry_run" = "yes" ]; then
    note "DRY RUN: would attach 8 PDFs and 4 Markdown guides, then publish"
  else
    # Globbed rather than named, so a guide that is renamed or added is carried without
    # editing this list, and counted below so one that vanishes is not.
    pdfs=(docs/quickstart/quick-start-*.pdf)
    markdown=("$guides_markdown"/quick-start-*.md)
    [ "${#pdfs[@]}" -eq 8 ] || { echo "found ${#pdfs[@]} guide PDFs, expected 8" >&2; exit 1; }
    [ "${#markdown[@]}" -eq 4 ] || { echo "found ${#markdown[@]} markdown guides, expected 4" >&2; exit 1; }

    gh_ release upload "$tag" --repo DrAlexHarrison/plateforce --clobber \
      "${pdfs[@]}" "${markdown[@]}"

    gh_ release edit "$tag" --repo DrAlexHarrison/plateforce --draft=false --latest

    # Counted against the declared set rather than trusted: a release missing a platform
    # is perfectly valid to anything that reads only what is present.
    attached="$(gh_ release view "$tag" --repo DrAlexHarrison/plateforce --json assets --jq '.assets|length')"
    [ "$attached" -eq 21 ] || { echo "the release carries ${attached} assets, expected 21" >&2; exit 1; }
    note "published, ${attached} of 21 assets"
  fi
  finished "published"
fi

# ---------------------------------------------------------------- homebrew

if ! done_already "homebrew"; then
  say "Carrying ${version} to Homebrew"

  tap="$(brew --repository 2>/dev/null)/Library/Taps/dralexharrison/homebrew-tap"
  if [ ! -d "$tap" ]; then
    note "the tap is not on this machine, skipping. brew tap dralexharrison/tap to add it"
  elif [ "$dry_run" = "yes" ]; then
    note "DRY RUN: would rewrite the formula's three urls and digests"
  else
    downloads="https://github.com/DrAlexHarrison/plateforce/releases/download/${tag}"
    scratch="$(mktemp -d)"
    for asset in plateforce-universal-macos plateforce-x86_64-linux-static plateforce-aarch64-linux-static; do
      curl -sSL -o "$scratch/$asset" "$downloads/$asset"
      # A digest taken from a truncated download is a formula nobody can install, and a
      # short read looks exactly like a small file, so the size is asserted against what
      # the release says it published.
      published_size="$(gh_ release view "$tag" --repo DrAlexHarrison/plateforce \
        --json assets --jq ".assets[] | select(.name==\"$asset\") | .size")"
      actual_size="$(wc -c < "$scratch/$asset" | tr -d ' ')"
      [ "$published_size" = "$actual_size" ] \
        || { echo "$asset: downloaded ${actual_size} of ${published_size} bytes" >&2; exit 1; }
    done

    python3 - "$tap/Formula/plateforce.rb" "$version" \
      "$(shasum -a 256 "$scratch/plateforce-universal-macos" | cut -d' ' -f1)" \
      "$(shasum -a 256 "$scratch/plateforce-x86_64-linux-static" | cut -d' ' -f1)" \
      "$(shasum -a 256 "$scratch/plateforce-aarch64-linux-static" | cut -d' ' -f1)" <<'PYTHON'
import re, sys, pathlib
formula, version, macos, linux_x64, linux_arm64 = sys.argv[1:6]
path = pathlib.Path(formula)
text = path.read_text()

text = re.sub(r"/download/v[0-9.]+/", f"/download/v{version}/", text)

# Each digest is replaced beside the url that names its own asset, so two assets can never
# take each other's checksum.
for asset, digest in (
    ("plateforce-universal-macos", macos),
    ("plateforce-x86_64-linux-static", linux_x64),
    ("plateforce-aarch64-linux-static", linux_arm64),
):
    text = re.sub(
        rf'(url "[^"]*{re.escape(asset)}"\n(\s*)sha256 ")[0-9a-f]{{64}}(")',
        lambda m: m.group(1) + digest + m.group(3),
        text,
    )

path.write_text(text)
remaining = re.findall(r"/download/v([0-9.]+)/", text)
if set(remaining) != {version}:
    raise SystemExit(f"the formula still names {sorted(set(remaining))}")
print(f"    formula rewritten to {version}, {len(remaining)} urls")
PYTHON

    ruby -c "$tap/Formula/plateforce.rb" >/dev/null
    rm -rf "$scratch"

    git -C "$tap" add -A
    git -C "$tap" -c user.name="Alex Harrison" -c user.email="alex@saturdaymorning.fit" \
      commit -q -m "plateforce ${version}"
    git -C "$tap" push -q origin main
    git -C "$tap" ls-remote --heads origin main | grep -aq . \
      || { echo "the tap push did not land" >&2; exit 1; }
    note "tap updated and pushed"

    # The formula is proven by installing it, not by auditing it. A reader's first act is
    # this command, so it is this command that has to work.
    brew uninstall plateforce >/dev/null 2>&1 || true
    brew install dralexharrison/tap/plateforce >/dev/null
    installed="$(plateforce version)"
    [ "$installed" = "plateforce ${version}" ] \
      || { echo "brew installed '$installed'" >&2; exit 1; }
    note "brew install gives ${installed}"
  fi
  finished "homebrew"
fi

# ---------------------------------------------------------------- the handover

if ! done_already "handover"; then
  say "Building the folder to hand over"

  folder="$HOME/plateforce-v${version}"
  if [ "$dry_run" = "yes" ]; then
    note "DRY RUN: would build $folder"
  else
    rm -rf "$folder"
    mkdir -p "$folder/Install/macOS" "$folder/Install/Windows" "$folder/Install/Linux" "$folder/Guides"
    scratch="$(mktemp -d)"
    gh_ release download "$tag" --repo DrAlexHarrison/plateforce --dir "$scratch"

    mv "$scratch"/*universal.dmg "$scratch"/plateforce-universal-macos "$folder/Install/macOS/"
    mv "$scratch"/*x64-setup.exe "$scratch"/plateforce-x86_64-windows.exe "$folder/Install/Windows/"
    mv "$scratch"/*.deb "$scratch"/*.rpm "$scratch"/*.AppImage \
       "$scratch"/plateforce-*-linux-static "$folder/Install/Linux/"
    mv "$scratch"/quick-start-* "$folder/Guides/"
    rm -rf "$scratch"

    # Checksum lines and nothing else. A title above them is read as a malformed entry by
    # the very command the folder tells a reader to run, so the check answers with a
    # warning about improper formatting beside its own passes. What the file is belongs in
    # the note that explains the folder.
    ( cd "$folder" && find Install Guides -type f | sort | while read -r file; do
        printf '%s  %s\n' "$(shasum -a 256 "$file" | cut -d' ' -f1)" "$file"
      done > CHECKSUMS.txt )

    # The note that says what the folder is and which file to open first. Carried forward
    # from the last release and restamped, because a folder handed to somebody in person
    # arrives without the release page around it.
    previous="$(ls -d "$HOME"/plateforce-v* 2>/dev/null | grep -av "$version" | tail -1)"
    if [ -n "$previous" ] && [ -f "$previous/CONTENTS.txt" ]; then
      sed -e "s/[0-9]\{1,\}\.[0-9]\{1,\}\.[0-9]\{1,\}/${version}/g" \
        "$previous/CONTENTS.txt" > "$folder/CONTENTS.txt"
      note "CONTENTS.txt carried forward from $(basename "$previous") and restamped"
    else
      echo "no CONTENTS.txt to carry forward; write one into ${folder}" >&2
    fi

    checked="$( cd "$folder" && shasum -a 256 -c CHECKSUMS.txt 2>/dev/null | grep -ac 'OK$' )"
    [ "$checked" -eq 21 ] \
      || { echo "${checked} of 21 files in ${folder} match their checksum" >&2; exit 1; }
    note "${checked} of 21 files verify against their own checksums"

    note "$folder, $(find "$folder" -type f | wc -l | tr -d ' ') files"
  fi
  finished "handover"
fi

say "plateforce ${version}"
note "https://github.com/DrAlexHarrison/plateforce/releases/tag/${tag}"
note "brew install dralexharrison/tap/plateforce"
note "pip install --upgrade plateforce"
[ "$dry_run" = "no" ] && rm -f "$state_file"
