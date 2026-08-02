#!/usr/bin/env bash
#
# Runs a program and reports every network address it tried to reach.
#
#   ./scripts/verify-no-outbound-request.sh <seconds> <command> [args...]
#
# The product's claim is that a researcher's trace never leaves their machine, and the
# header the reader sees says so. This measures the claim directly, by watching the
# connect() calls the program makes, rather than inferring it from a program that survived
# having its network taken away. A local socket to the display server or the session bus is
# not an outbound request; an address that is not loopback is.

set -uo pipefail

if [ "$#" -lt 2 ]; then
  echo "usage: $0 <seconds> <command> [args...]" >&2
  exit 2
fi

seconds="$1"
shift

if ! command -v strace >/dev/null 2>&1; then
  echo "strace is not here, so what the program reaches cannot be read" >&2
  exit 2
fi

trace="$(mktemp)"
trap 'rm -f "$trace"' EXIT

# Its own process group, so the whole tree goes at the end. Signalling strace alone leaves
# the program it is tracing alive and holding the terminal, which reads as a hung check.
setsid strace -f -e trace=connect -o "$trace" "$@" >/dev/null 2>&1 &
watched=$!
sleep "$seconds"
kill -TERM -"$watched" 2>/dev/null || kill -TERM "$watched" 2>/dev/null
sleep 1
kill -KILL -"$watched" 2>/dev/null
wait "$watched" 2>/dev/null

# A local socket carries no address off the machine. Loopback is the browser talking to the
# copy of the interface this same program is serving.
outbound="$(grep -E 'sa_family=AF_INET6?' "$trace" \
  | grep -v 'inet_addr("127\.' \
  | grep -v '"::1"' || true)"

total="$(grep -c 'connect(' "$trace" || true)"
local_sockets="$(grep -c 'sa_family=AF_UNIX' "$trace" || true)"

if [ -n "$outbound" ]; then
  echo "the program reached for an address off this machine:" >&2
  printf '%s\n' "$outbound" | head -5 >&2
  exit 1
fi

echo "no outbound request in ${seconds}s: ${total} connections, ${local_sockets} to local sockets, 0 to any address"
