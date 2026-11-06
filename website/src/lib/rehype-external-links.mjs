// Ends every docs text link that leaves the site with "&nbsp;↗", the landing's mark for an outside link (the arrow
// is aria-hidden; screen readers hear the link text alone). Links on the site's own origin, links whose text
// already ends in ↗, links with no text (an image or icon) and links inside headings or code stay as written.
// Handles Markdown links and <a> written as JSX in an .mdx page.

const EXTERNAL = /^https?:\/\//i;
const SKIP_INSIDE = /^(h[1-6]|pre|code)$/;

const nameOf = (node) => (node.type === "element" ? node.tagName : node.name);
const isJsx = (node) => node.type === "mdxJsxTextElement" || node.type === "mdxJsxFlowElement";

function hrefOf(node) {
  if (node.type === "element") return node.properties?.href;
  return node.attributes?.find((attribute) => attribute.name === "href")?.value;
}

function textOf(node) {
  if (node.type === "text") return node.value;
  return (node.children ?? []).map(textOf).join("");
}

/** The arrow, as the same kind of node as the link so both the Markdown and the MDX pipelines render it. */
function arrowFor(link) {
  if (!isJsx(link)) return { type: "element", tagName: "span", properties: { ariaHidden: "true" }, children: [{ type: "text", value: "↗" }] };
  const attributes = [{ type: "mdxJsxAttribute", name: "aria-hidden", value: "true" }];
  return { type: "mdxJsxTextElement", name: "span", attributes, children: [{ type: "text", value: "↗" }] };
}

export default function rehypeExternalLinks({ site } = {}) {
  const ownOrigin = site ? new URL(site).origin : null;
  const leavesSite = (href) => typeof href === "string" && EXTERNAL.test(href) && new URL(href).origin !== ownOrigin;

  return (tree) => {
    const walk = (parent) => {
      for (const node of parent.children ?? []) {
        if (node.type !== "element" && !isJsx(node)) continue;
        if (SKIP_INSIDE.test(nameOf(node))) continue;
        if (nameOf(node) === "a" && leavesSite(hrefOf(node))) {
          const text = textOf(node).trim();
          if (text && !text.endsWith("↗")) node.children.push({ type: "text", value: " " }, arrowFor(node));
          continue;
        }
        walk(node);
      }
    };
    walk(tree);
  };
}
