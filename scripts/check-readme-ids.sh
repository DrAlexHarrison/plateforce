#!/usr/bin/env bash
# Every dotted token printed in a README must be an id a reader can look up.
#
# A published example is the artefact this project exists to make trustworthy, and an id
# in one that resolves nowhere sends a reader to a registry that has never held it.
#
# Extraction is total rather than clever. A pattern demanding two or more dots misses
# jump_height.from_takeoff_velocity, and a pattern taking one or more picks up every Python
# attribute chain in the worked examples, so the exclusions are a committed file a reviewer
# reads instead of a regex nobody can check.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

readmes=(README.md crates/plateforce-python/README.md)
allowlist=docs/readme-id-allowlist.txt

for file in "${readmes[@]}" "$allowlist"; do
    if [ ! -f "$file" ]; then
        echo "plateforce: $file is missing" >&2
        exit 1
    fi
done

# A file name is dotted the same way an id is, so `docs/for-an-agent.md` reaches the reader of
# this pattern as `agent.md`. The last segment tells them apart, and the assertion below proves
# the exclusion cannot reach a published id rather than trusting that it does not.
file_endings='md|json|csv|toml|txt|rs|py|js|sh|html|zip|png|pdf|yml|yaml|lock'

tokens="$(grep -rhoE "\b[a-z][a-z_0-9]*(\.[a-z][a-z_0-9]*)+\b" "${readmes[@]}" \
    | grep -vE "\.($file_endings)\$" | sort -u)"
permitted="$(grep -vE '^\s*(#|$)' "$allowlist" | sort -u)"

# An id that does not resolve and a lookup that cannot run report the same way, so an
# unbuildable workspace would name every published id as missing and send somebody to
# correct a README against a compile error. Prove the lookup works before believing a miss.
if ! cargo run -q -p plateforce-cli -- registry show bwepoch.fixed_window >/dev/null 2>&1; then
    echo "plateforce: registry show cannot resolve bwepoch.fixed_window, so the lookup is" >&2
    echo "            unavailable and no README claim can be checked against it" >&2
    exit 70
fi

# Without the fallback this pipeline ends the script under `set -e` with nothing said, and a
# guard that dies silently reads in a log exactly like a step that ran and found nothing.
published="$(cargo run -q -p plateforce-cli -- methods --format json 2>/dev/null \
    | grep -oE '"[a-z][a-z_0-9]*(\.[a-z][a-z_0-9]*)+"' | tr -d '"' | sort -u || true)"
published_count="$(printf '%s\n' "$published" | grep -c . || true)"
if [ "${published_count:-0}" -lt 40 ]; then
    echo "plateforce: read ${published_count:-0} published ids from methods --format json, too" >&2
    echo "            few for the file-ending exclusion to have been tested against anything" >&2
    exit 70
fi
swallowed="$(printf '%s\n' "$published" | grep -E "\.($file_endings)\$" || true)"
if [ -n "$swallowed" ]; then
    echo "plateforce: the file-ending exclusion would hide these published ids:" >&2
    printf '  %s\n' $swallowed >&2
    exit 70
fi

status=0

# An allowlist entry no README prints any more is a licence to hide a real id later.
while IFS= read -r entry; do
    [ -n "$entry" ] || continue
    if ! printf '%s\n' "$tokens" | grep -qxF "$entry"; then
        echo "plateforce: $allowlist permits $entry, which no README prints" >&2
        status=1
    fi
done <<<"$permitted"

checked=0
unresolved=0
while IFS= read -r token; do
    [ -n "$token" ] || continue
    if printf '%s\n' "$permitted" | grep -qxF "$token"; then
        continue
    fi
    checked=$((checked + 1))
    if ! cargo run -q -p plateforce-cli -- registry show "$token" >/dev/null 2>&1; then
        echo "plateforce: no registry entry with id $token" >&2
        unresolved=$((unresolved + 1))
        status=1
    fi
done <<<"$tokens"

# A control, so a zero means every id resolved rather than that nothing was looked up.
if [ "$checked" -eq 0 ]; then
    echo "plateforce: the READMEs printed no ids to check, which is itself the defect" >&2
    exit 1
fi
if cargo run -q -p plateforce-cli -- registry show definitely.not.an.entry >/dev/null 2>&1; then
    echo "plateforce: the registry resolved an id that does not exist, so a pass means nothing" >&2
    exit 1
fi

echo "$((checked - unresolved)) of $checked ids printed in the READMEs resolve"
exit "$status"
