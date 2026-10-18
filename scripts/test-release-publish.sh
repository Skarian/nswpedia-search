#!/bin/bash
# Dry run of the release workflow's publish job against a scratch repository,
# so publishing problems show up in a minute instead of after a release run.
# Runs the "Check assets", "Release notes" and "Publish release" steps of
# .github/workflows/release.yml verbatim, with dummy assets bundled by
# scripts/bundle-assets.sh.
#
#   bash scripts/test-release-publish.sh [owner/scratch-repo]
#
# Default scratch repo: Skarian/nswpedia-release-test (private; created if
# missing). Needs gh logged in with the repo scope. Test releases and tags are
# deleted afterwards, so the scratch repo can be reused.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
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
# A checkout like the workflow's: this repo's .github and scripts, committed
# and pushed to the scratch repo so the release can target the commit.
src="$work/src"
mkdir -p "$src"
cp -r .github scripts "$src/"
git -C "$src" init -q -b main
git -C "$src" add .
git -C "$src" -c user.name=release-test -c user.email=release-test@localhost commit -qm "test $n"
git -C "$src" push -q --force "https://github.com/$repo.git" main
sha=$(git -C "$src" rev-parse HEAD)
tree=$(git -C "$src" rev-parse 'HEAD^{tree}')

cleanup() {
  for t in "v$stable" "v$rc"; do gh release delete "$t" -R "$repo" --yes --cleanup-tag >/dev/null 2>&1 || true; done
  rm -rf "$work"
}
trap cleanup EXIT
fail() { echo "FAILED: $*" >&2; exit 1; }
tag_exists() { gh api "repos/$repo/git/ref/tags/$1" >/dev/null 2>&1; }

publish() { # version prerelease built-from-commit
  rm -rf "$src/release-assets"
  mkdir -p "$src/release-assets"
  for f in windows-x64-setup.exe windows-x64-portable.zip macos-arm64.dmg macos-x64.dmg; do
    echo "$1" > "$src/release-assets/nswpedia-search-$1-$f"
  done
  bash "$root/scripts/bundle-assets.sh" create "$src/release-assets" "$1" "$tree" "$3" 1 >/dev/null
  (
    cd "$src"
    export VERSION=$1 PRERELEASE=$2 TREE=$tree RUNNER_TEMP=$work GITHUB_SHA=$sha \
      GITHUB_SERVER_URL=https://github.com GITHUB_REPOSITORY=$repo GH_REPO=$repo
    bash -e "$work/Check-assets.sh" >/dev/null
    bash -e "$work/Release-notes.sh"
    bash -e "$work/Publish-release.sh"
  )
}

tag_exists "v$stable" && fail "test tag already exists"
# An interrupted run leaves at most a draft, which must not create the tag.
gh release create "v$stable" -R "$repo" --draft --target "$sha" --notes draft >/dev/null
tag_exists "v$stable" && fail "a draft release created the tag"
echo "ok   a draft release does not create the tag"
publish "$stable" false "$sha"
publish "$rc" true 0000000000000000000000000000000000000000

info=$(gh release view "v$stable" -R "$repo" --json isDraft,isPrerelease,assets \
  --jq '"\(.isDraft) \(.isPrerelease) \([.assets[].name] | sort | join(","))"')
expected="false false SHA256SUMS.txt,nswpedia-search-$stable-macos-arm64.dmg,nswpedia-search-$stable-macos-x64.dmg,nswpedia-search-$stable-windows-x64-portable.zip,nswpedia-search-$stable-windows-x64-setup.exe"
[[ $info == "$expected" ]] || fail "stable release: got '$info'"
echo "ok   stable release published with the four installers and SHA256SUMS.txt"
[[ $(gh api "repos/$repo/git/ref/tags/v$stable" --jq .object.sha) == "$sha" ]] || fail "tag does not point at the release commit"
echo "ok   publishing created tag v$stable at the release commit"
[[ $(gh release view "v$rc" -R "$repo" --json isDraft,isPrerelease --jq '"\(.isDraft) \(.isPrerelease)"') == "false true" ]] \
  || fail "rc release is not a published prerelease"
echo "ok   rc release published as prerelease"
[[ $(gh api "repos/$repo/releases/latest" --jq .tag_name) == "v$stable" ]] || fail "Latest is not v$stable"
echo "ok   stable is Latest; rc did not replace it"
gh release view "v$stable" -R "$repo" --json body --jq .body | grep -q "from commit $sha." || fail "notes lack provenance"
gh release view "v$rc" -R "$repo" --json body --jq .body | grep -q "identical source tree $tree" \
  || fail "notes for assets built from another commit don't say so"
echo "ok   notes state where the assets were built (and the identical tree when commits differ)"
if publish "$stable" false "$sha" >/dev/null 2>&1; then fail "re-running a published version did not refuse"; fi
echo "ok   re-running a published version refuses"
echo x >> "$src/release-assets/nswpedia-search-$stable-macos-x64.dmg"
if (cd "$src" && VERSION=$stable TREE=$tree bash -e "$work/Check-assets.sh") >/dev/null 2>&1; then
  fail "a modified asset passed the check"
fi
echo "ok   a modified asset is rejected before publishing"
echo "Release publish dry run passed."
