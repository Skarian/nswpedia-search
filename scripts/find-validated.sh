#!/bin/bash
# Finds a reusable artifact for an exact source tree and prints its run ID.
#   bash scripts/find-validated.sh validated <tree>       checks passed
#   bash scripts/find-validated.sh release-assets <tree>  tested installers
# Exits 1 if none qualifies. An artifact counts only if it is unexpired, its
# run completed successfully, the run executed this repository's own
# ci.yml/build.yml (not a fork pull request, which runs the fork's workflow
# files), and the artifact name carries the same git tree hash.
# Needs GH_TOKEN (actions: read) and GITHUB_REPOSITORY.
set -euo pipefail
kind=$1 tree=$2
repo=${GITHUB_REPOSITORY:?}
[[ $tree =~ ^[0-9a-f]{40}$ ]] || { echo "Bad tree hash '$tree'" >&2; exit 2; }
repo_id=$(gh api "repos/$repo" --jq .id)
candidates=$(gh api "repos/$repo/actions/artifacts?name=$kind-$tree&per_page=100" \
  --jq ".artifacts[] | select(.expired | not)
        | select(.workflow_run.repository_id == $repo_id and .workflow_run.head_repository_id == $repo_id)
        | .workflow_run.id")
for run in $candidates; do
  ok=$(gh api "repos/$repo/actions/runs/$run" --jq \
    '(.conclusion == "success") and (.path == ".github/workflows/ci.yml" or .path == ".github/workflows/build.yml")')
  if [[ $ok == true ]]; then
    echo "$run"
    exit 0
  fi
done
echo "No successful $kind run for tree $tree" >&2
exit 1
