// @vitest-environment node
import { describe, expect, it } from "vitest";
import rehypeExternalLinks from "./rehype-external-links.mjs";

const SITE = "https://harness-kit.dev";
const text = (value: string) => ({ type: "text", value });
const element = (tagName: string, children: object[] = [], properties: object = {}) => ({ type: "element", tagName, properties, children });
const link = (href: string, ...children: object[]) => element("a", children, { href });
const arrow = [text(" "), element("span", [text("↗")], { ariaHidden: "true" })];

function run(children: object[]) {
  const tree = { type: "root", children };
  rehypeExternalLinks({ site: SITE })(tree);
  return tree.children as any[];
}

describe("rehypeExternalLinks", () => {
  it("ends a link that leaves the site with a non-breaking space and an aria-hidden arrow", () => {
    const [p] = run([element("p", [text("Get the "), link("https://github.com/RealZST/HarnessKit/releases/latest", text("latest release"))])]);
    expect(p.children[1].children).toEqual([text("latest release"), ...arrow]);
  });

  it("marks plain http links too", () => {
    const [a] = run([link("http://example.com", text("example"))]);
    expect(a.children).toHaveLength(3);
  });

  it("leaves links within the docs and on the site's own origin as written", () => {
    const [docs, own, anchor] = run([link("/docs/faq/", text("FAQ")), link(`${SITE}/docs/`, text("Docs")), link("#setup", text("Setup"))]);
    for (const a of [docs, own, anchor]) expect(a.children).toHaveLength(1);
  });

  it("does not add a second arrow to a link that already ends in one", () => {
    const [a] = run([link("https://example.com", text("example "), element("span", [text("↗")], { ariaHidden: "true" }))]);
    expect(a.children).toHaveLength(2);
  });

  it("leaves image-only links alone", () => {
    const [a] = run([link("https://example.com", element("img", [], { src: "x.png", alt: "Example" }))]);
    expect(a.children).toHaveLength(1);
  });

  it("leaves links inside headings and code alone", () => {
    const [h2, pre] = run([element("h2", [link("https://example.com", text("Example"))]), element("pre", [element("code", [link("https://example.com", text("url"))])])]);
    expect(h2.children[0].children).toHaveLength(1);
    expect(pre.children[0].children[0].children).toHaveLength(1);
  });

  it("marks a JSX <a> in an .mdx page with a JSX arrow, so MDX renders it", () => {
    const attributes = [{ type: "mdxJsxAttribute", name: "href", value: "https://smithery.ai" }];
    const [a] = run([{ type: "mdxJsxTextElement", name: "a", attributes, children: [text("Smithery")] }]);
    expect(a.children[2]).toMatchObject({ type: "mdxJsxTextElement", name: "span", attributes: [{ name: "aria-hidden", value: "true" }], children: [text("↗")] });
  });
});
