#!/bin/bash
# Bundles tested release assets, or verifies a bundle before publishing.
#
#   bash scripts/bundle-assets.sh create <dir> <version> <tree> <commit> <run-id>
#       <dir> must hold exactly the four installers for <version>. Writes
#       SHA256SUMS.txt and provenance.txt (version, source tree, commit and
#       workflow run the files were built and tested in).
#   bash scripts/bundle-assets.sh verify <dir> <version> <tree>
#       Fails unless <dir> holds exactly the four installers plus those two
#       files, every checksum matches, and provenance names the same version
#       and source tree. Prints the provenance.
set -euo pipefail
mode=$1 dir=$2 version=$3 tree=$4
files=(
  "nswpedia-search-$version-windows-x64-setup.exe"
  "nswpedia-search-$version-windows-x64-portable.zip"
  "nswpedia-search-$version-macos-arm64.dmg"
  "nswpedia-search-$version-macos-x64.dmg"
)
fail() { echo "::error::$*" >&2; exit 1; }
[[ $tree =~ ^[0-9a-f]{40}$ ]] || fail "Bad tree hash '$tree'"
cd "$dir"
for f in "${files[@]}"; do [[ -s $f ]] || fail "Missing $f"; done

case $mode in
  create)
    commit=$5 run=$6
    [[ $(ls | wc -l) -eq ${#files[@]} ]] || fail "Unexpected files: $(ls | tr '\n' ' ')"
    sha256sum "${files[@]}" > SHA256SUMS.txt
    printf 'VERSION=%s\nTREE=%s\nCOMMIT=%s\nRUN_ID=%s\nREPOSITORY=%s\n' \
      "$version" "$tree" "$commit" "$run" "${GITHUB_REPOSITORY:-local}" > provenance.txt
    ;;
  verify)
    [[ $(ls | wc -l) -eq $((${#files[@]} + 2)) ]] || fail "Unexpected files: $(ls | tr '\n' ' ')"
    sha256sum --check --strict SHA256SUMS.txt
    listed=$(awk '{ sub(/^\*/, "", $2); print $2 }' SHA256SUMS.txt | sort)
    [[ $listed == "$(printf '%s\n' "${files[@]}" | sort)" ]] || fail "SHA256SUMS.txt does not list exactly the installers"
    field() { sed -n "s/^$1=//p" provenance.txt; }
    [[ $(field VERSION) == "$version" ]] || fail "Assets are version '$(field VERSION)', expected $version"
    [[ $(field TREE) == "$tree" ]] || fail "Assets were built from tree $(field TREE), not $tree"
    ;;
  *) fail "Unknown mode '$mode'" ;;
esac
cat provenance.txt
