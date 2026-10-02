# rlch/takumi

A fork of [kane50613/takumi](https://github.com/kane50613/takumi) for Tutero's
schools-ts render service. `master` tracks upstream untouched; `rlch` is the
integration branch: upstream `master` plus the commits below, rebased, never
merged. Every fix here is meant to go upstream; drop its commit once it lands.

## What differs

- **@property registrations once per stylesheet** (`takumi-core`, `style/custom_properties.rs`):
  registered custom properties are collected once per render and shared, as
  Stylo does, instead of being re-applied on every element. ~13x faster renders
  with Tailwind v4's 63 registrations.
- **An invalid declaration is dropped, not thrown** (`takumi-core`,
  `style/stylesheets.rs`, `style/css_source.rs`): a style object's or a css rule
  object's declaration whose value its property does not take (a wrong type, a
  value that does not parse, one Takumi does not implement such as
  `contain: strict`) is dropped and the rest applies, as CSS does. Upstream
  throws and fails the render.
- **takumi-pdf's wasm is built for speed** (`takumi-pdf-js/speed.toml`): the
  release profile's size overrides on the PDF graph go back to opt-level 3 for
  this build only, and the release builds std without `optimize_for_size`.
  1.3-1.6x faster per document, PDFs byte-identical, the .wasm ~6.3 MB (~2.4 MB
  gzip): over upstream's 2 MiB Worker ceiling, so this takumi-pdf is for Node,
  not for a Worker. @takumi-rs/wasm and takumi-paint keep upstream's sizes;
  @takumi-rs/core (napi) keeps upstream's profile.
- **Release pipeline** (`.github/workflows/rlch-release.yml`,
  `scripts/rlch-version.sh`, `scripts/rlch-pack.sh`): ci.yml's build jobs, on a
  tag, for the targets schools-ts runs on (darwin-arm64, linux-arm64-gnu,
  linux-x64-gnu), packed as npm tarballs onto a GitHub Release.

## The npm packages

Each package is published under the `@rlch` scope, and each dependency on
another is an npm alias that installs it under its upstream name (the code
imports `@takumi-rs/core`; napi's loader requires `@takumi-rs/core-<platform>`):

| upstream                     | on npm                         |
| ---------------------------- | ------------------------------ |
| `takumi-js`                  | `@rlch/takumi-js`              |
| `takumi-pdf`                 | `@rlch/takumi-pdf`             |
| `@takumi-rs/core`            | `@rlch/takumi-core`            |
| `@takumi-rs/core-<platform>` | `@rlch/takumi-core-<platform>` |
| `@takumi-rs/helpers`         | `@rlch/takumi-helpers`         |
| `@takumi-rs/wasm`            | `@rlch/takumi-wasm`            |

A consumer depends on `"takumi-js": "npm:@rlch/takumi-js@<version>"`.

## Sync with upstream

```sh
git fetch upstream                     # upstream = https://github.com/kane50613/takumi
git switch master && git merge --ff-only upstream/master && git push origin master
git switch rlch && git rebase upstream/master
git push --force-with-lease origin rlch
```

Never push to upstream; a fix goes there as a pull request from its own branch.

## Cut a release

The tag is the takumi line's version on `rlch` (`takumi-js/package.json`) plus
`-rlch.<n>`; every package gets the same suffix on its own version (takumi-pdf
`0.15.0` ships as `0.15.0-rlch.<n>`).

```sh
git tag v2.14.0-rlch.3 rlch && git push origin v2.14.0-rlch.3
gh run watch -R rlch/takumi            # rlch release: builds, packs, releases
```

Then publish the release's tarballs, as the npm account `rlch`:

```sh
gh release download v2.14.0-rlch.3 -R rlch/takumi -D release
for f in release/*.tgz; do npm publish "$f" --access public --tag rlch; done
```

(`--tag rlch` keeps `latest` free.) In schools-ts, change the versions in
`pnpm-workspace.yaml`'s `overrides` and the `package.json`s that name a takumi
package, and run `pnpm install`.
