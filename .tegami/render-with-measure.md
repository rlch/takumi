---
packages:
  "takumi": minor
---

# Render and measure in one layout pass

`Renderer.renderWithMeasure()` returns `{ image, measured }` and `Renderer.renderSvgWithMeasure()` returns `{ svg, measured }`: the output `render()` or `renderSvg()` draws and the tree `measure()` returns, from one layout instead of two. In Rust, `takumi::render_with_measure` and `takumi::render_svg_with_measure`.
