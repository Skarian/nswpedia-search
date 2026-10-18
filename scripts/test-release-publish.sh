#!/bin/bash
# Dry run of the release workflow's publish steps against a scratch repository,
# so publishing problems show up in a minute instead of after a 30-minute
# release run. Runs the "Check assets", "Release notes" and "Publish release"
# steps exactly as written in .github/workflows/release.yml, with dummy files.
#
#   bash scripts/test-release-publish.sh [owner/scratch-repo]
#
# Default scratch repo: Skarian/nswpedia-release-test (private; created if
# missing). Needs gh logged in with the repo scope. Test releases and tags are
# deleted afterwards, so the scratch repo can be reused.
set -euo pipefail
cd "$(dirname "$0")/.."
repo=${1:-Skarian/nswpedia-release-test}
workflow=.github/workflows/release.yml
work=$(mktemp -d)
n=$(date +%s)
stable="0.0.$n" rc="0.0.$((n + 1))-rc.1"

# Body of a step's `run: |` block.
step() {
  awk -v name="name: $1" 'index($0, name) { f = 1; next }
    f && /^      - / { exit }
    f && /run: \|/ { r = 1; next }
    f && r { print }' "$workflow" | sed 's/^          //'
}
for s in "Check assets" "Release notes" "Publish release"; do
  step "$s" > "$work/${s// /-}.sh"
  [[ -s "$work/${s// /-}.sh" ]] || { echo "Step '$s' not found in $workflow" >&2; exit 1; }
done

if ! gh repo view "$repo" >/dev/null 2>&1; then
  gh repo create "$repo" --private --description "Scratch repo for scripts/test-release-publish.sh" >/dev/null
fi
git -C "$work" init -q -b main
echo "Release workflow test $n" > "$work/README.md"
git -C "$work" add README.md
git -C "$work" -c user.name=release-test -c user.email=release-test@localhost commit -qm "test $n"
git -C "$work" push -q --force "https://github.com/$repo.git" main
git -C "$work" tag "v$stable"
git -C "$work" tag "v$rc"
git -C "$work" push -q "https://github.com/$repo.git" "v$stable" "v$rc"

cleanup() {
  for t in "v$stable" "v$rc"; do gh release delete "$t" -R "$repo" --yes --cleanup-tag >/dev/null 2>&1 || true; done
  rm -rf "$work"
}
trap cleanup EXIT

publish() { # tag version prerelease
  local dir="$work/$1"
  mkdir -p "$dir/release-assets"
  cp -r .github "$dir/"
  for f in windows-x64-setup.exe windows-x64-portable.zip macos-arm64.dmg macos-x64.dmg; do
    echo "$1" > "$dir/release-assets/nswpedia-search-$2-$f"
  done
  (
    cd "$dir"
    export VERSION=$2 PRERELEASE=$3 RUNNER_TEMP=$dir GITHUB_REF_NAME=$1 GH_REPO=$repo
    (cd release-assets && bash -e "$work/Check-assets.sh" >/dev/null)
    bash -e "$work/Release-notes.sh"
    bash -e "$work/Publish-release.sh"
  )
}
fail() { echo "FAILED: $*" >&2; exit 1; }

publish "v$stable" "$stable" false
publish "v$rc" "$rc" true

info=$(gh release view "v$stable" -R "$repo" --json isDraft,isPrerelease,assets \
  --jq '"\(.isDraft) \(.isPrerelease) \([.assets[].name] | sort | join(","))"')
expected="false false SHA256SUMS.txt,nswpedia-search-$stable-macos-arm64.dmg,nswpedia-search-$stable-macos-x64.dmg,nswpedia-search-$stable-windows-x64-portable.zip,nswpedia-search-$stable-windows-x64-setup.exe"
[[ $info == "$expected" ]] || fail "stable release: got '$info'"
echo "ok   stable release published with all assets"
[[ $(gh release view "v$rc" -R "$repo" --json isDraft,isPrerelease --jq '"\(.isDraft) \(.isPrerelease)"') == "false true" ]] \
  || fail "rc release is not a published prerelease"
echo "ok   rc release published as prerelease"
[[ $(gh api "repos/$repo/releases/latest" --jq .tag_name) == "v$stable" ]] || fail "Latest is not v$stable"
echo "ok   stable is Latest; rc did not replace it"
if publish "v$stable" "$stable" false >/dev/null 2>&1; then fail "re-running a published tag did not refuse"; fi
echo "ok   re-running a published tag refuses"
echo "Release publish dry run passed."
