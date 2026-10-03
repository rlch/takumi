---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint a scaled or rotated box from its rounded position, as Chrome does

- A box with a transform that is not a translation, such as `scale(0.5)`, now paints from its position rounded to a whole pixel, as Chrome's paint offset translation does. Its pictures no longer resample half a pixel off and blur when it sits at a fractional position.
- A box with `contain: paint` paints from its rounded position too.
