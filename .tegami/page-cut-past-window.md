---
"takumi-pdf": patch
---

# Move a box that ends past the content window to the next page

A page cut landed on any content edge within a pixel of it, so a box ending up to a pixel past the window stayed on the page and overfilled it. An edge past the window now takes the cut only within a layout unit (1/64px), which still fills a page exactly with a box sized to a fractional window.
