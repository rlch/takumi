import { describe, expect, it } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { Renderer } from "../src/export";

describe("Renderer.renderWithMeasure", () => {
  const renderer = new Renderer();
  const node = container({
    style: {
      display: "flex",
      flexDirection: "column",
      gap: 8,
      width: 240,
      padding: 12,
      backgroundColor: "#eef",
    },
    children: [
      text({ text: "Hello, world", style: { fontSize: 24 } }),
      container({ style: { width: 120, height: 40, backgroundColor: "red" } }),
    ],
  });

  it("returns render's image and measure's tree", async () => {
    const options = { width: 300, height: 200, format: "png" } as const;
    const { image, measured } = await renderer.renderWithMeasure(node, options);

    expect(image).toEqual(await renderer.render(node, options));
    expect(measured).toEqual(await renderer.measure(node, options));
  });

  it("returns raw pixels when asked for them", async () => {
    const options = { width: 300, height: 200, format: "raw" } as const;
    const { image } = await renderer.renderWithMeasure(node, options);

    expect(image).toEqual(await renderer.render(node, options));
  });
});

describe("Renderer.renderSvgWithMeasure", () => {
  const renderer = new Renderer();
  const node = container({
    style: { width: 200, padding: 10, backgroundColor: "#fee" },
    children: [text({ text: "Measured once", style: { fontSize: 20 } })],
  });

  it("returns renderSvg's document and measure's tree", async () => {
    const options = { width: 220, height: 120 };
    const { svg, measured } = await renderer.renderSvgWithMeasure(node, options);

    expect(svg).toBe(await renderer.renderSvg(node, options));
    expect(measured).toEqual(await renderer.measure(node, options));
  });
});
