//! The main renderer for Takumi image rendering engine.

use std::{collections::HashMap, sync::Arc};

use base64::{Engine, prelude::BASE64_STANDARD};
use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{
  FontStore, css_or_stylesheets, device_pixel_ratio,
  input::{decode_images, register_font},
  parse_lang, stylesheet, time_ms,
};
use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::image::{ImageSource as LoadedImageSource, ResourceCache},
  style::FontFamily,
  viewport::Viewport,
};
use takumi_raster::{
  AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFormat, Bitmap,
  SequentialScene, measure, render, render_with_measure, write_animation, write_image,
};
use wasm_bindgen::prelude::*;

use crate::{helper::map_error, model::*};

/// The main renderer for Takumi image rendering engine.
///
/// State lives behind a lock and every method takes `&self`, mirroring the
/// napi bindings: a panic mid-call can't leave the wasm-bindgen borrow flag
/// permanently set, which would otherwise fail all subsequent calls.
#[wasm_bindgen]
pub struct Renderer {
  fonts: FontStore,
  resource_cache: ResourceCache,
}

fn raster_options<'fonts>(
  resource_cache: &ResourceCache,
  fonts: &'fonts Fonts,
  node: Node,
  options: RenderOptions,
  images: HashMap<Arc<str>, LoadedImageSource>,
) -> Result<takumi_raster::RenderOptions<'fonts>, js_sys::Error> {
  let stylesheet = stylesheet(
    resource_cache,
    css_or_stylesheets(options.css, options.stylesheets),
    options.keyframes.unwrap_or_default(),
  )
  .map_err(map_error)?;
  let lang = parse_lang(options.lang.as_deref()).map_err(map_error)?;

  Ok(
    takumi_raster::RenderOptions::builder()
      .viewport(
        Viewport::new((options.width, options.height))
          .with_device_pixel_ratio(device_pixel_ratio(options.device_pixel_ratio)),
      )
      .draw_debug_border(options.draw_debug_border.unwrap_or_default())
      .images(images)
      .stylesheet(stylesheet)
      .time_ms(time_ms(options.time_ms))
      .dithering(options.dithering.unwrap_or_default())
      .node(node)
      .fonts(fonts)
      .font_families(options.font_families.map(FontFamily::from_names))
      .lang(lang)
      .build(),
  )
}

#[wasm_bindgen]
impl Renderer {
  fn images_map(
    &self,
    images: Option<&[ImageSource]>,
  ) -> Result<HashMap<Arc<str>, LoadedImageSource>, js_sys::Error> {
    decode_images(&self.resource_cache, images.unwrap_or_default()).map_err(map_error)
  }

  /// Creates a new Renderer instance.
  #[wasm_bindgen(constructor)]
  pub fn new(options: Option<RendererOptionsType>) -> Result<Renderer, js_sys::Error> {
    let options: RendererOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    Ok(Renderer {
      fonts: FontStore::new().map_err(map_error)?,
      resource_cache: match options.cache_max_bytes {
        Some(bytes) => ResourceCache::new(bytes),
        None => ResourceCache::default(),
      },
    })
  }

  /// Registers fonts into the renderer, returning the families each font produced.
  #[wasm_bindgen(js_name = registerFont)]
  pub fn register_font(&self, font: FontType) -> Result<RegisteredFamiliesType, js_sys::Error> {
    let font: Font = from_value(font.into()).map_err(map_error)?;

    let mut state = self.fonts.write().map_err(map_error)?;
    let registered = register_font(&mut state, font).map_err(map_error)?;

    Ok(to_value(&registered).map_err(map_error)?.unchecked_into())
  }

  /// Renders a node tree into an image buffer.
  #[wasm_bindgen(unchecked_return_type = "Uint8Array<ArrayBuffer>")]
  pub fn render(
    &self,
    node: NodeType,
    options: Option<RenderOptionsType>,
  ) -> Result<Vec<u8>, JsValue> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let images = self.images_map(options.images.as_deref())?;
    let state = self.fonts.read().map_err(map_error)?;
    self.render_internal(&state, node, options, images)
  }

  fn render_internal(
    &self,
    fonts: &Fonts,
    node: Node,
    options: RenderOptions,
    images: HashMap<Arc<str>, LoadedImageSource>,
  ) -> Result<Vec<u8>, JsValue> {
    let format = options.format.unwrap_or(OutputFormat::Png);
    let quality = options.quality;
    let render_options = raster_options(&self.resource_cache, fonts, node, options, images)?;

    let image = render(render_options).map_err(map_error)?;

    Ok(encode(image, format, quality)?)
  }

  /// Renders a node tree into an image buffer and measures its layout, laying it out once for
  /// both. The image is `render`'s and the measured tree `measure`'s for the same options.
  #[wasm_bindgen(js_name = renderWithMeasure)]
  pub fn render_with_measure(
    &self,
    node: NodeType,
    options: Option<RenderOptionsType>,
  ) -> Result<RenderedWithMeasureType, JsValue> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let format = options.format.unwrap_or(OutputFormat::Png);
    let quality = options.quality;
    let images = self.images_map(options.images.as_deref())?;
    let state = self.fonts.read().map_err(map_error)?;
    let render_options = raster_options(&self.resource_cache, &state, node, options, images)?;

    let (image, measured) = render_with_measure(render_options).map_err(map_error)?;
    let image = js_sys::Uint8Array::from(encode(image, format, quality)?.as_slice());

    with_measure("image", image.into(), &measured)
  }

  /// Renders a node tree into an SVG document string.
  #[wasm_bindgen(js_name = renderSvg)]
  pub fn render_svg(
    &self,
    node: NodeType,
    options: Option<SvgRenderOptionsType>,
  ) -> Result<String, JsValue> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: SvgRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let images = self.images_map(options.images.as_deref())?;
    let state = self.fonts.read().map_err(map_error)?;

    Ok(takumi_svg::render(self.svg_options(&state, node, options, images)?).map_err(map_error)?)
  }

  /// Renders a node tree into an SVG document string and measures its layout, laying it out once
  /// for both. The SVG is `renderSvg`'s and the measured tree `measure`'s for the same options.
  #[wasm_bindgen(js_name = renderSvgWithMeasure)]
  pub fn render_svg_with_measure(
    &self,
    node: NodeType,
    options: Option<SvgRenderOptionsType>,
  ) -> Result<SvgRenderedWithMeasureType, JsValue> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: SvgRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let images = self.images_map(options.images.as_deref())?;
    let state = self.fonts.read().map_err(map_error)?;

    let (svg, measured) =
      takumi_svg::render_with_measure(self.svg_options(&state, node, options, images)?)
        .map_err(map_error)?;

    with_measure("svg", svg.into(), &measured)
  }

  fn svg_options<'fonts>(
    &self,
    fonts: &'fonts Fonts,
    node: Node,
    options: SvgRenderOptions,
    images: HashMap<Arc<str>, LoadedImageSource>,
  ) -> Result<takumi_svg::SvgOptions<'fonts>, js_sys::Error> {
    let stylesheet = stylesheet(
      &self.resource_cache,
      css_or_stylesheets(options.css, options.stylesheets),
      options.keyframes.unwrap_or_default(),
    )
    .map_err(map_error)?;
    let lang = parse_lang(options.lang.as_deref()).map_err(map_error)?;

    Ok(
      takumi_svg::SvgOptions::builder()
        .viewport(Viewport::new((options.width, options.height)))
        .images(images)
        .stylesheet(stylesheet)
        .time_ms(time_ms(options.time_ms))
        .node(node)
        .fonts(fonts)
        .font_families(options.font_families.map(FontFamily::from_names))
        .lang(lang)
        .build(),
    )
  }

  /// Measures a node tree and returns layout information.
  #[wasm_bindgen(js_name = measure)]
  pub fn measure(
    &self,
    node: NodeType,
    options: Option<RenderOptionsType>,
  ) -> Result<MeasuredNodeType, JsValue> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let images = self.images_map(options.images.as_deref())?;

    let state = self.fonts.read().map_err(map_error)?;
    let render_options = raster_options(&self.resource_cache, &state, node, options, images)?;

    let layout = measure(render_options).map_err(map_error)?;

    Ok(to_value(&layout).map_err(map_error)?.into())
  }

  /// Renders a node tree into a data URL.
  ///
  /// `raw` format is not supported for data URL.
  #[wasm_bindgen(js_name = "renderAsDataUrl")]
  pub fn render_as_data_url(
    &self,
    node: NodeType,
    options: RenderOptionsType,
  ) -> Result<String, js_sys::Error> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let options: RenderOptions = from_value(options.into()).map_err(map_error)?;

    let format = options.format.unwrap_or(OutputFormat::Png);

    if format == OutputFormat::Raw {
      return Err(js_sys::Error::new(
        "Raw format is not supported for data URL",
      ));
    }

    let images = self.images_map(options.images.as_deref())?;
    let state = self.fonts.read().map_err(map_error)?;
    let buffer = self.render_internal(&state, node, options, images)?;

    let mut data_uri = String::new();

    data_uri.push_str("data:");
    data_uri.push_str(format.into_image_output_format(None).content_type());
    data_uri.push_str(";base64,");
    data_uri.push_str(&BASE64_STANDARD.encode(buffer));

    Ok(data_uri)
  }

  /// Renders a sequential animation timeline into a buffer.
  #[wasm_bindgen(js_name = renderAnimation, unchecked_return_type = "Uint8Array<ArrayBuffer>")]
  pub fn render_animation(&self, options: RenderAnimationOptionsType) -> Result<Vec<u8>, JsValue> {
    let RenderAnimationOptions {
      scenes,
      width,
      height,
      format,
      images,
      draw_debug_border,
      css,
      stylesheets,
      keyframes,
      device_pixel_ratio: dpr,
      fps,
      font_families,
      lang,
    } = from_value(options.into()).map_err(map_error)?;

    let lang = parse_lang(lang.as_deref()).map_err(map_error)?;

    let images = self.images_map(images.as_deref())?;

    if scenes.is_empty() {
      return Err(JsValue::from_str("Expected at least one animation scene"));
    }

    if fps == 0 {
      return Err(JsValue::from_str("Expected fps to be greater than 0"));
    }

    let viewport = Viewport::new((width, height)).with_device_pixel_ratio(device_pixel_ratio(dpr));
    let draw_debug_border = draw_debug_border.unwrap_or_default();
    let stylesheet = stylesheet(
      &self.resource_cache,
      css_or_stylesheets(css, stylesheets),
      keyframes.unwrap_or_default(),
    )
    .map_err(map_error)?;
    let state = self.fonts.read().map_err(map_error)?;
    let scene_options = scenes
      .into_iter()
      .map(|scene| {
        SequentialScene::builder()
          .duration_ms(scene.duration_ms)
          .options(
            takumi_raster::RenderOptions::builder()
              .viewport(viewport)
              .images(images.clone())
              .stylesheet(stylesheet.clone())
              .node(scene.node)
              .fonts(&state)
              .font_families(font_families.clone().map(FontFamily::from_names))
              .lang(lang)
              .draw_debug_border(draw_debug_border)
              .build(),
          )
          .build()
      })
      .collect::<Vec<_>>();

    // wasm `image-webp` is lossless-only, which the WebP option defaults to.
    let format = match format.unwrap_or(AnimationOutputFormat::WebP) {
      AnimationOutputFormat::WebP => AnimationFormat::WebP(AnimatedWebpOptions::default()),
      AnimationOutputFormat::APng => AnimationFormat::Apng(AnimatedPngOptions::default()),
      AnimationOutputFormat::Gif => AnimationFormat::Gif(AnimatedGifOptions::default()),
    };

    let mut buffer = Vec::new();
    write_animation(&scene_options, fps, format, &mut buffer).map_err(map_error)?;

    Ok(buffer)
  }
}

/// Encodes `image` in `format`; `raw` is its RGBA pixels.
fn encode(
  image: Bitmap,
  format: OutputFormat,
  quality: Option<u8>,
) -> Result<Vec<u8>, js_sys::Error> {
  if format == OutputFormat::Raw {
    return Ok(image.into_raw());
  }

  let mut buffer = Vec::new();

  write_image(
    &image,
    &mut buffer,
    format.into_image_output_format(quality),
  )
  .map_err(map_error)?;

  Ok(buffer)
}

/// `{ [key]: output, measured }`, the result of a `…WithMeasure` call.
fn with_measure<T: JsCast>(
  key: &str,
  output: JsValue,
  measured: &takumi_raster::MeasuredNode,
) -> Result<T, JsValue> {
  let result = js_sys::Object::new();

  js_sys::Reflect::set(&result, &key.into(), &output)?;
  js_sys::Reflect::set(
    &result,
    &"measured".into(),
    &to_value(measured).map_err(map_error)?,
  )?;

  Ok(result.unchecked_into())
}
