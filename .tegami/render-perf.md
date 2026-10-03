---
packages:
  "takumi": patch
---

# Render faster

- Shadows, backdrop filters and `blur()` blur faster, with NEON, SSE2 or wasm simd128 on the alpha pass.
- Text is shaped and measured once per node, and glyph positions, `text-decoration-skip-ink` intercepts and `line-height: normal` metrics are reused within a render.
- Oblique linear gradients, scaled images, masks and the final alpha pass skip per-pixel work.
- `@property` rules are collected once per render instead of applied to every element, and a child shares its parent's custom properties until it sets one, so a page styled with Tailwind v4's stylesheet renders several times faster.
- A `font-family` stack is expanded against the registered subset families once per render instead of once per text run, strut and decoration, so a page whose fallback chain names CJK families split into many `unicode-range` slices lays out its inline content much faster.
