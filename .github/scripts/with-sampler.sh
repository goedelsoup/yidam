#!/usr/bin/env bash
#
# Run a command while streaming resource samples to stdout.
#
# When a GitHub-hosted runner is killed for memory, nothing in the log names the resource:
# the step ends with "The runner has received a shutdown signal" and an exit code, and memory
# and disk are equally good guesses that take opposite fixes. #539 measured one such kill by
# adding this loop to the failing step; the next kill landed one step earlier, where the loop
# was not, and left the same empty evidence a second time.
#
# So it lives here, and every heavy step calls it. `ci_reporting.rs` asserts that — a step
# added to `cli-full` without it is a step whose next kill is undiagnosable.
#
# stdout, deliberately. A file under /tmp dies with the VM, and a post-step with
# `if: always()` does not run when the runner agent is the thing that was killed. The last
# line to reach the log survives precisely the failure it explains.
set -uo pipefail

(
  while true; do
    printf '[res] %s mem_avail=%s disk_avail=%s\n' \
      "$(date -u +%H:%M:%S)" \
      "$(awk '/MemAvailable/{printf "%.1fGi", $2/1048576}' /proc/meminfo)" \
      "$(df -h --output=avail / | tail -1 | tr -d ' ')"
    sleep 15
  done
) &
sampler=$!
trap 'kill "$sampler" 2>/dev/null || true' EXIT

# Last command, so the script exits with the status of the work rather than the trap.
"$@"
