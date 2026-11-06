// @vitest-environment node
import { describe, expect, it } from "vitest";
import rehypeTableRegions from "./rehype-table-regions.mjs";

const text = (value: string) => ({ type: "text", value });
const h2 = (title: string) => ({ type: "element", tagName: "h2", properties: {}, children: [text(title)] });
const table = () => ({ type: "element", tagName: "table", properties: {}, children: [] });
const jsxTable = () => ({ type: "mdxJsxFlowElement", name: "table", attributes: [], children: [] });

function run(children: object[]) {
  const tree = { type: "root", children };
  rehypeTableRegions()(tree);
  return tree.children as any[];
}

describe("rehypeTableRegions", () => {
  it("wraps a Markdown table in a focusable region named after the heading above it", () => {
    const [, region] = run([h2("hk list"), table()]);
    expect(region).toMatchObject({
      tagName: "div",
      properties: { className: ["table-region"], role: "region", tabIndex: 0, ariaLabel: "hk list" },
      children: [{ tagName: "table" }],
    });
  });

  it("wraps a JSX table in a JSX region, so MDX renders the wrapper too", () => {
    const [, region] = run([h2("All agents"), jsxTable()]);
    expect(region.type).toBe("mdxJsxFlowElement");
    expect(region.name).toBe("div");
    expect(region.attributes).toContainEqual({ type: "mdxJsxAttribute", name: "aria-label", value: "All agents" });
    expect(region.children[0].name).toBe("table");
  });

  it("numbers the labels of several tables under one heading, so each region has its own name", () => {
    const regions = run([h2("Options"), table(), table()]).slice(1);
    expect(regions.map((r) => r.properties.ariaLabel)).toEqual(["Options", "Options (2)"]);
  });

  it("falls back to 'Table' before the first heading", () => {
    expect(run([table()])[0].properties.ariaLabel).toBe("Table");
  });

  it("finds tables nested in other elements, such as asides", () => {
    const aside = { type: "element", tagName: "aside", properties: {}, children: [table()] };
    expect(run([h2("Limitations"), aside])[1].children[0].properties.role).toBe("region");
  });
});
