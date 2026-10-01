#!/bin/sh
# The forced command for each key in authorized_keys: `gate.sh read` or `gate.sh write`.
#
# A read key may fetch. Only a write key may push. Anything that is not git is refused.
mode="$1"
case "$SSH_ORIGINAL_COMMAND" in
  git-upload-pack\ *) ;;
  git-receive-pack\ *)
    if [ "$mode" != write ]; then
      echo "gate: a $mode key cannot push" >&2
      exit 1
    fi
    ;;
  *)
    echo "gate: not a git command" >&2
    exit 1
    ;;
esac
exec git-shell -c "$SSH_ORIGINAL_COMMAND"
