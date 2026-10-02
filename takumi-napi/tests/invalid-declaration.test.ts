import { expect, test } from "bun:test";
import { container, type Declarations } from "@takumi-rs/helpers";
import { Renderer } from "../src/export";

const renderer = new Renderer();

const base: Declarations = { width: "60px", height: "40px", backgroundColor: "#832ec5" };

async function draw(style: Declarations) {
  return Buffer.from(
    await renderer.render(container({ children: [], style }), { width: 100, height: 100 }),
  );
}

// CSS drops a declaration whose value its property does not take (css-syntax-3
// § 8), and the rest of the style stands. A wrong type, a value that does not
// parse, and a value the property is not implemented for all read that way.
const invalid: Declarations[] = [
  // @ts-expect-error: invalid type test
  { justifyContent: 123 },
  { justifyContent: "star" },
  { color: "notacolor" },
  // @ts-expect-error: invalid type test
  { width: true },
  { width: "invalid" },
  { borderRadius: "10px / invalid" },
  // @ts-expect-error: invalid type test
  { padding: { top: null } },
  { gap: "invalid" },
  { textDecorationLine: "invalid" },
  { contain: "strict" },
  { contain: "size layout" },
];

for (const declaration of invalid) {
  test(`drops ${JSON.stringify(declaration)} and draws the rest`, async () => {
    const rest = Object.fromEntries(
      Object.entries(base).filter(([name]) => !(name in declaration)),
    ) as Declarations;

    expect(await draw({ ...base, ...declaration })).toEqual(await draw(rest));
  });
}
