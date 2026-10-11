#!/usr/bin/env bash
# Runs a command. If it fails, shows its first errors as annotations of the
# run, where they can be read without opening the whole log.
#
# Usage: run-and-report.sh <title> <command> [arguments...]
set -uo pipefail

title="$1"
shift
log="$(mktemp)"

"$@" 2>&1 | tee "$log"
status="${PIPESTATUS[0]}"
if [ "$status" -ne 0 ]; then
    # From the first error on, a screenful. `%` and line ends are written
    # the way workflow commands expect them.
    message="$(awk '/^error/ { seen = 1 } seen' "$log" | head -n 120 \
        | sed -e 's/%/%25/g' -e 's/\r//g' | awk '{ printf "%s%%0A", $0 }')"
    if [ -z "$message" ]; then
        message="$(tail -n 60 "$log" | sed -e 's/%/%25/g' -e 's/\r//g' | awk '{ printf "%s%%0A", $0 }')"
    fi
    echo "::error title=${title}::${message}"
fi
exit "$status"
