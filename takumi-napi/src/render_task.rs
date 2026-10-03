use std::{collections::HashMap, mem::take, sync::Arc};

use napi::bindgen_prelude::*;
use takumi_bindings_common::{device_pixel_ratio, parse_lang, stylesheet, time_ms};
use takumi_core::{
  layout::node::Node,
  style::{FontFamily, Lang, StyleSheet},
  viewport::Viewport,
};
use takumi_raster::{Bitmap, DitheringAlgorithm, render, render_with_measure, write_image};

use crate::{
  JsBytes, map_error,
  renderer::{
    ImageCacheMode, OutputFormat, RenderOptions, RenderedWithMeasure, RendererState,
    collect_images, decode_images, deserialize_css, deserialize_keyframes,
  },
};

pub struct RenderTask {
  pub(crate) draw_debug_border: bool,
  pub(crate) node: Option<Node>,
  pub(crate) state: Arc<RendererState>,
  pub(crate) viewport: Viewport,
  pub(crate) format: OutputFormat,
  pub(crate) quality: Option<u8>,
  pub(crate) lossless: Option<bool>,
  pub(crate) dithering: DitheringAlgorithm,
  pub(crate) time_ms: u64,
  pub(crate) stylesheet: Arc<StyleSheet>,
  pub(crate) images: HashMap<Arc<str>, (JsBytes, ImageCacheMode)>,
  pub(crate) font_families: Option<FontFamily>,
  pub(crate) lang: Option<Lang>,
}

impl RenderTask {
  pub(crate) fn from_options(
    env: Env,
    node: Node,
    options: RenderOptions,
    state: Arc<RendererState>,
  ) -> Result<Self> {
    let stylesheet = stylesheet(
      &state.resource_cache,
      deserialize_css(options.css, options.stylesheets)?,
      deserialize_keyframes(options.keyframes)?,
    )
    .map_err(map_error)?;

    Ok(RenderTask {
      node: Some(node),
      state,
      viewport: Viewport::new((options.width, options.height)).with_device_pixel_ratio(
        device_pixel_ratio(options.device_pixel_ratio.map(|ratio| ratio as f32)),
      ),
      format: options.format.unwrap_or(OutputFormat::Png),
      quality: options.quality,
      lossless: options.lossless,
      dithering: options.dithering.map(Into::into).unwrap_or_default(),
      time_ms: time_ms(options.time_ms),
      draw_debug_border: options.draw_debug_border.unwrap_or_default(),
      stylesheet,
      images: collect_images(env, options.images)?,
      font_families: options.font_families.map(FontFamily::from_names),
      lang: parse_lang(options.lang.as_deref()).map_err(map_error)?,
    })
  }
}

impl RenderTask {
  /// Runs `draw` over this task's raster options. Takes the node, so it runs once.
  fn draw<T>(
    &mut self,
    draw: impl FnOnce(takumi_raster::RenderOptions) -> takumi_core::Result<T>,
  ) -> Result<T> {
    let Some(node) = self.node.take() else {
      unreachable!()
    };

    let fonts = self.state.fonts.load();

    let initialized_images = decode_images(&self.state.resource_cache, take(&mut self.images))?;

    draw(
      takumi_raster::RenderOptions::builder()
        .viewport(self.viewport)
        .images(initialized_images)
        .resource_cache(self.state.resource_cache.clone())
        .stylesheet(take(&mut self.stylesheet))
        .time_ms(self.time_ms)
        .dithering(self.dithering)
        .node(node)
        .fonts(&fonts)
        .font_families(take(&mut self.font_families))
        .lang(take(&mut self.lang))
        .draw_debug_border(self.draw_debug_border)
        .build(),
    )
    .map_err(map_error)
  }

  /// Encodes `image` in this task's output format.
  fn encode(&self, image: Bitmap) -> Result<Vec<u8>> {
    if self.format == OutputFormat::Raw {
      return Ok(image.into_raw());
    }

    let mut buffer = Vec::new();

    write_image(
      &image,
      &mut buffer,
      self
        .format
        .into_image_output_format(self.quality, self.lossless),
    )
    .map_err(map_error)?;

    Ok(buffer)
  }
}

impl Task for RenderTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    let image = self.draw(render)?;

    self.encode(image)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// A [`RenderTask`] that also returns the measured layout, from the same layout pass.
pub struct RenderWithMeasureTask(pub(crate) RenderTask);

impl Task for RenderWithMeasureTask {
  type Output = (Vec<u8>, takumi_raster::MeasuredNode);
  type JsValue = RenderedWithMeasure;

  fn compute(&mut self) -> Result<Self::Output> {
    let (image, measured) = self.0.draw(render_with_measure)?;

    Ok((self.0.encode(image)?, measured))
  }

  fn resolve(&mut self, _env: Env, (image, measured): Self::Output) -> Result<Self::JsValue> {
    Ok(RenderedWithMeasure {
      image: image.into(),
      measured: measured.into(),
    })
  }
}
