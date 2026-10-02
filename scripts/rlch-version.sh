#!/usr/bin/env bash
# rlch fork: put a release tag's version on every package the release ships.
#
#   scripts/rlch-version.sh v2.14.0-rlch.1
#
# The tag names the takumi line's version; its prerelease suffix (`-rlch.1`) goes
# on every package's own version, so takumi-pdf 0.15.0 ships as 0.15.0-rlch.1,
# and each `workspace:*` dependency becomes the version it now points at.
# Idempotent. Run it after `bun install --frozen-lockfile` (the lockfile records
# upstream's versions) and before a build that bakes the version in: napi's
# loader compares its platform package's version with its own.
set -euo pipefail

tag="${1:?tag, e.g. v2.14.0-rlch.1}"
version="${tag#v}"
base="${version%%-*}"
suffix="${version#"$base"}"
case "$suffix" in -rlch.*) ;; *) echo "tag $tag is not v<version>-rlch.<n>" >&2; exit 1 ;; esac

js_base="$(jq -r .version takumi-js/package.json)"
if [ "${js_base%%-*}" != "$base" ]; then
  echo "tag $tag is for $base, but takumi-js on this commit is $js_base" >&2
  exit 1
fi

packages=(takumi-helpers takumi-napi takumi-wasm takumi-js takumi-pdf-js)

for dir in "${packages[@]}"; do
  file="$dir/package.json"
  own="$(jq -r .version "$file")"
  jq --arg v "${own%%-*}$suffix" '.version = $v' "$file" > "$file.tmp" && mv "$file.tmp" "$file"
done

versions="$(for dir in "${packages[@]}"; do jq -c '{(.name): .version}' "$dir/package.json"; done | jq -s -c 'add')"
for dir in "${packages[@]}"; do
  file="$dir/package.json"
  jq --argjson v "$versions" '
    if .dependencies then
      .dependencies |= with_entries(if (.value | startswith("workspace:")) then .value = $v[.key] else . end)
    else . end' "$file" > "$file.tmp" && mv "$file.tmp" "$file"
done

echo "$versions"
