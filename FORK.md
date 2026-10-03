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
- **A scaled box paints from its rounded paint offset** (`takumi-core`,
  `scene.rs`, `style/stylesheets_query.rs`): the fraction of a transformed
  box's position that Blink's paint offset translation drops no longer moves
  its device space, so a picture under `scale()` at a fractional position
  draws on the pixel grid instead of a fraction off and blurred. Regressed
  upstream in #1780.
- **A font-family stack expands once per render** (`takumi-core`,
  `font_style.rs`, `context.rs`): instead of once per text run, strut and
  decoration, each a copy of every registered subset slice's name. Regressed
  upstream in #1774, which added the per-box strut metrics: a maths page's
  thumbnail laid out 40-50% slower than on 2.13.5.
- **An inline image parses once per render, and once per renderer cache**
  (`takumi-core`, `layout/node/image.rs`, `context.rs`, and every binding): a
  data URI, SVG markup or raw bytes in `src` resolved through the render
  context's table and the renderer's `ResourceCache`, not re-parsed by each
  layout pass and the paint; takumi-pdf shares one cache across pages. A
  `currentColor` SVG keeps its re-parse per host color. A maths page (formulas
  as SVG data URIs) draws ~5x faster, its PDF ~3x, output byte-identical.
- **Render and measure from one layout pass** (`takumi-raster`, `takumi-svg`,
  napi, wasm): `renderWithMeasure` / `renderSvgWithMeasure` return the image or
  SVG and the `MeasuredNode`, laying the tree out once where `measure` then
  `render` lay it out twice. Output and measured tree identical to the two
  calls (a test holds every HTML fixture to it).
- **A box that ends past the content window moves to the next page**
  (`takumi-pdf`, `pagination.rs`): a page cut landed on any content edge within
  a pixel of it, so a box ending up to a pixel past the window stayed on the
  page and overfilled it (a worksheet's sheet held 1048 px on a 1047 px area).
  An edge past the window takes the cut only within a layout unit (1/64px).
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
  linux-x64-gnu), packed as npm tarballs onto a GitHub Release, then published
  to npm by trusted publishing (OIDC): no npm token exists anywhere.

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
gh run watch -R rlch/takumi            # rlch release: builds, packs, releases, publishes
```

The `publish` job (environment `npm`) downloads the release's tarballs and runs
`npm publish <tgz> --access public --tag rlch` for each, platform packages
first, with the job's GitHub OIDC token: npm's trusted publishing, provenance
included. Nobody publishes by hand. The GitHub environment `npm` admits
only `v*-rlch.*` tags, so no branch run can reach it.

Each of the 8 packages trusts exactly this: owner `rlch`, repository `takumi`,
workflow `rlch-release.yml`, environment `npm` (npmjs.com › the package ›
Settings › Trusted Publisher, or `npm trust github <pkg> --repo rlch/takumi
--file rlch-release.yml --env npm --allow-publish`, npm 11.15+). Renaming the
workflow or the environment, or adding a package, needs that set again first;
a new package must exist on npm before it can be trusted, so its first version
is published by hand (`npm publish <tgz> --access public --tag rlch
--auth-type=web`).

If the publish job fails, fix the cause and re-run that job (`gh run rerun
<run-id> --failed -R rlch/takumi`): a version already on npm is skipped, so it
publishes only what is missing. A `404`/`E403` naming OIDC means the package's
trusted publisher does not match the workflow file or environment above.

(`--tag rlch` keeps `latest` free.) In schools-ts, change the versions in
`pnpm-workspace.yaml`'s `overrides` and the `package.json`s that name a takumi
package, and run `pnpm install`.
