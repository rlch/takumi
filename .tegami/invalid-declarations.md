---
packages:
  "takumi": minor
---

# Drop an invalid declaration instead of failing the render

- A `style` object's declaration whose value its property does not take (a wrong type, a value that does not parse, or one Takumi does not implement, such as `contain: strict`) is dropped and the rest of the style applies, as CSS drops an invalid declaration. It used to throw and fail the render, so markup a browser draws, with `contain: size layout` inline, could not be drawn at all.
- A `css` rule object drops such a declaration the same way instead of rejecting the whole rule.
