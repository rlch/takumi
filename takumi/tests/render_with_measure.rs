//! `render_with_measure` and `render_svg_with_measure` lay a tree out once for
//! both its picture and its measurement, so each must return exactly what the
//! two separate calls do: the same pixels or SVG, and the same measured tree.
//! Every HTML fixture is held to that.

mod test_utils;

use std::result::Result;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use takumi::{
  measure, prelude::*, render, render_svg, render_svg_with_measure, render_with_measure,
};
use test_utils::{html_fixture, html_fixture_paths};

fn svg_options(options: &RenderOptions<'static>) -> SvgOptions<'static> {
  SvgOptions::builder()
    .node(options.node().clone())
    .viewport(*options.viewport())
    .fonts(options.fonts())
    .stylesheet(options.stylesheet().clone())
    .images(options.images().clone())
    .build()
}

/// The fixtures whose combined call disagrees with the separate ones, each with why.
fn mismatches(check: impl Fn(RenderOptions<'static>) -> Result<(), String> + Sync) -> Vec<String> {
  html_fixture_paths()
    .par_iter()
    .filter_map(|path| {
      let name = path.file_stem().unwrap().to_string_lossy();

      html_fixture(path)
        .and_then(&check)
        .err()
        .map(|error| format!("{name}: {error}"))
    })
    .collect()
}

#[test]
fn render_with_measure_matches_render_and_measure() {
  let failures = mismatches(|options| {
    let separate = render(options.clone()).map(|image| (image, measure(options.clone())));
    let combined = render_with_measure(options);

    match (separate, combined) {
      (Ok((image, measured)), Ok((combined_image, combined_measured))) => {
        if image.as_raw() != combined_image.as_raw() {
          return Err("the image differs from render's".into());
        }
        if measured.as_ref().ok() != Some(&combined_measured) {
          return Err("the measured tree differs from measure's".into());
        }
        Ok(())
      }
      (Err(_), Err(_)) => Ok(()),
      (separate, combined) => Err(format!(
        "render {} but render_with_measure {}",
        if separate.is_ok() { "drew" } else { "failed" },
        if combined.is_ok() { "drew" } else { "failed" },
      )),
    }
  });

  assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn render_svg_with_measure_matches_render_svg_and_measure() {
  let failures = mismatches(|options| {
    let separate = render_svg(svg_options(&options)).map(|svg| (svg, measure(options.clone())));
    let combined = render_svg_with_measure(svg_options(&options));

    match (separate, combined) {
      (Ok((svg, measured)), Ok((combined_svg, combined_measured))) => {
        if svg != combined_svg {
          return Err("the SVG differs from render_svg's".into());
        }
        if measured.as_ref().ok() != Some(&combined_measured) {
          return Err("the measured tree differs from measure's".into());
        }
        Ok(())
      }
      (Err(_), Err(_)) => Ok(()),
      (separate, combined) => Err(format!(
        "render_svg {} but render_svg_with_measure {}",
        if separate.is_ok() { "drew" } else { "failed" },
        if combined.is_ok() { "drew" } else { "failed" },
      )),
    }
  });

  assert!(failures.is_empty(), "{failures:#?}");
}
