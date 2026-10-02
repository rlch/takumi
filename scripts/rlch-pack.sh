#!/usr/bin/env bash
# rlch fork: pack every package the release ships, as .tgz files.
#
#   scripts/rlch-pack.sh <tag> <out-dir>     e.g. scripts/rlch-pack.sh v2.14.0-rlch.1 release
#
# Run from the repository root after the built artifacts are in place, as
# upstream's publish job lays them out: takumi-helpers/dist, takumi-napi/{dist,
# index.js,index.d.ts}, takumi-napi/artifacts/bindings-<target>/core.*.node,
# takumi-wasm/{pkg,dist}, takumi-js/dist, takumi-pdf-js/{pkg,dist}. The tarballs
# are what `npm publish` takes (FORK.md).
set -euo pipefail

tag="${1:?tag, e.g. v2.14.0-rlch.1}"
out="$(mkdir -p "${2:?out dir}" && cd "$2" && pwd)"
root="$(pwd)"

bash "$root/scripts/rlch-version.sh" "$tag"

# @takumi-rs/core's per-platform packages: upstream's `bun artifacts` (napi
# create-npm-dirs, artifacts, version), for the targets the release built (one
# `artifacts/bindings-<target>` each, the workflow's matrix). The
# optionalDependencies napi's pre-publish would write are written here, so no
# step can reach the npm registry.
(
  cd takumi-napi
  targets="$(ls -d artifacts/bindings-*/ | sed -E 's#artifacts/bindings-(.*)/#\1#' | jq -R . | jq -s -c .)"
  if [ "$targets" = "[]" ]; then echo "no napi artifacts under takumi-napi/artifacts" >&2; exit 1; fi
  jq --argjson t "$targets" '.napi.targets = $t' package.json > package.json.tmp && mv package.json.tmp package.json
  bunx napi create-npm-dirs
  bunx napi artifacts
  bunx napi version
  deps="$(for pkg in npm/*/package.json; do jq -c '{(.name): .version}' "$pkg"; done | jq -s 'add')"
  jq --argjson deps "$deps" '.optionalDependencies = $deps' package.json > package.json.tmp
  mv package.json.tmp package.json
  for dir in npm/*/; do
    if ! ls "$dir"core.*.node > /dev/null 2>&1; then echo "no binary in $dir" >&2; exit 1; fi
  done
)

dirs=(takumi-napi/npm/*/ takumi-helpers takumi-napi takumi-wasm takumi-js takumi-pdf-js)

# Every package ships under the @rlch scope on npm (`takumi-js` as
# `@rlch/takumi-js`, `@takumi-rs/core-linux-x64-gnu` as
# `@rlch/takumi-core-linux-x64-gnu`), and each dependency on another of them is
# an npm alias that installs it under its upstream name: the code imports
# `@takumi-rs/core`, and napi's loader requires `@takumi-rs/core-<platform>`.
renames="$(for dir in "${dirs[@]}"; do jq -c '{(.name): ("@rlch/" + (.name | sub("^@takumi-rs/"; "takumi-")))}' "$root/$dir/package.json"; done | jq -s -c 'add')"
versions="$(for dir in "${dirs[@]}"; do jq -c '{(.name): .version}' "$root/$dir/package.json"; done | jq -s -c 'add')"
for dir in "${dirs[@]}"; do
  file="$root/$dir/package.json"
  jq --argjson r "$renames" --argjson v "$versions" '
    def alias: with_entries(if $r[.key] then .value = "npm:\($r[.key])@\($v[.key])" else . end);
    .name = $r[.name]
    | .repository = {type: "git", url: "git+https://github.com/rlch/takumi.git"}
    | .publishConfig = {access: "public"}
    | if .dependencies then .dependencies |= alias else . end
    | if .optionalDependencies then .optionalDependencies |= alias else . end' "$file" > "$file.tmp"
  mv "$file.tmp" "$file"
done

# `bun pm pack` resolves `catalog:` and honours `files`; the workspace
# dependencies were already written as versions above.
for dir in "${dirs[@]}"; do
  (cd "$root/$dir" && bun pm pack --quiet --destination "$out" > /dev/null)
done

ls -1 "$out"
