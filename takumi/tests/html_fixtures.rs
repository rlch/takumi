//! Renders every HTML file in `tests/fixtures-html/` into its raster and
//! vector goldens. The HTML is the fixture source: add a file, run this test,
//! eyeball the generated goldens. The same file opens in a browser for a
//! reference render (it links `shared.css` for the font faces).
//!
//! `oxfmt` formats the fixtures under `htmlWhitespaceSensitivity: strict`
//! (see `.oxfmtrc.json`), so it never adds or removes whitespace between
//! elements. Whitespace that is itself test content (preserve spans,
//! tab-size) is written as `&#9;`/`&#10;`/`&#32;` entities, which the
//! formatter cannot re-wrap.

mod test_utils;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use test_utils::{html_fixture, html_fixture_paths, write_goldens};

#[test]
fn html_fixtures() {
  let failures: Vec<String> = html_fixture_paths()
    .par_iter()
    .filter_map(|path| {
      let name = path.file_stem().unwrap().to_string_lossy();

      html_fixture(path)
        .and_then(|options| write_goldens(options, &name))
        .err()
        .map(|error| format!("{name}: {error}"))
    })
    .collect();

  assert!(failures.is_empty(), "{failures:#?}");
}
