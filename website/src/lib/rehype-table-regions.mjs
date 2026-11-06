// Wraps every docs table in a focusable, labelled region. A table wider than the column scrolls inside the
// region, and a keyboard user can tab to it and scroll with the arrow keys (axe: scrollable-region-focusable).
// The label is the nearest heading above the table, numbered when one section has several tables, so screen
// readers can tell the regions apart. Handles Markdown tables and <table> written as JSX in an .mdx page.

const HEADING = /^h[2-6]$/;

const isHeading = (node) => node.type === "element" && HEADING.test(node.tagName);
const isTable = (node) =>
  (node.type === "element" && node.tagName === "table") || (node.type === "mdxJsxFlowElement" && node.name === "table");

/** The heading's own text. An MDX expression such as {AGENT_COUNT} has no text yet, so keep counts out of headings. */
function textOf(node) {
  if (node.type === "text") return node.value;
  return (node.children ?? []).map(textOf).join("");
}

/** The wrapper, as the same kind of node as the table so both the Markdown and the MDX pipelines render it. */
function regionAround(table, label) {
  if (table.type === "element") {
    const properties = { className: ["table-region"], role: "region", tabIndex: 0, ariaLabel: label };
    return { type: "element", tagName: "div", properties, children: [table] };
  }
  const attributes = { class: "table-region", role: "region", tabindex: "0", "aria-label": label };
  return {
    type: "mdxJsxFlowElement",
    name: "div",
    attributes: Object.entries(attributes).map(([name, value]) => ({ type: "mdxJsxAttribute", name, value })),
    children: [table],
  };
}

export default function rehypeTableRegions() {
  return (tree, file) => {
    let heading = "";
    const labelsUsed = new Map();
    // A table before any heading is named for what it is, in the page's language (Chinese pages are under zh/).
    const fallback = /[\\/]content[\\/]docs[\\/]zh[\\/]/.test(file?.path ?? "") ? "表格" : "Table";
    const nextLabel = () => {
      const base = heading || fallback;
      const count = (labelsUsed.get(base) ?? 0) + 1;
      labelsUsed.set(base, count);
      return count === 1 ? base : `${base} (${count})`;
    };
    const walk = (parent) => {
      parent.children?.forEach((node, index) => {
        if (isHeading(node)) heading = textOf(node).trim();
        if (isTable(node)) parent.children[index] = regionAround(node, nextLabel());
        else walk(node);
      });
    };
    walk(tree);
  };
}
