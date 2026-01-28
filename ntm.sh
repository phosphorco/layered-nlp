#!/usr/bin/env bash
set -euo pipefail

# Always run NTM sessions rooted at this repo.
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
base_dir="$(dirname "$repo_dir")"
session_name="$(basename "$repo_dir")"

export NTM_PROJECTS_BASE="$base_dir"

if [[ $# -eq 0 ]]; then
  exec ntm spawn "$session_name" --cc=2
fi

case "$1" in
  spawn|attach|view|dashboard|status|send|interrupt|kill|zoom|copy|save|palette)
    cmd="$1"
    shift
    exec ntm "$cmd" "$session_name" "$@"
    ;;
  *)
    exec ntm "$@"
    ;;
esac
