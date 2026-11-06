// @vitest-environment node
import { describe, expect, it } from "vitest";
import { renderInline } from "./inline-code";

const text = (t: string) => ({ kind: "text", text: t });
const code = (t: string) => ({ kind: "code", text: t });

describe("renderInline", () => {
  it("passes plain text through as one text segment", () => {
    expect(renderInline("No markup here.")).toEqual([text("No markup here.")]);
  });
  it("returns no segments for an empty string", () => {
    expect(renderInline("")).toEqual([]);
  });
  it("splits out one backtick span as code", () => {
    expect(renderInline("Run `hk serve` on the server.")).toEqual([text("Run "), code("hk serve"), text(" on the server.")]);
  });
  it("handles several spans, including one at each end", () => {
    expect(renderInline("`SKILL.md` becomes `SKILL.md.disabled`")).toEqual([code("SKILL.md"), text(" becomes "), code("SKILL.md.disabled")]);
  });
  it("leaves an unbalanced backtick literal", () => {
    expect(renderInline("a `b c")).toEqual([text("a `b c")]);
    expect(renderInline("`x` and `y")).toEqual([code("x"), text(" and `y")]);
  });

  describe("links", () => {
    it("turns [text](href) into a link segment", () => {
      expect(renderInline("Start from the [adapter guide](/docs/contributing/).")).toEqual([
        text("Start from the "),
        { kind: "link", href: "/docs/contributing/", parts: [text("adapter guide")] },
        text("."),
      ]);
    });
    it("allows code spans inside the link text", () => {
      expect(renderInline("See [the `hk` CLI](/docs/cli/).")).toEqual([
        text("See "),
        { kind: "link", href: "/docs/cli/", parts: [text("the "), code("hk"), text(" CLI")] },
        text("."),
      ]);
    });
    it("accepts root-relative, fragment and https hrefs", () => {
      for (const href of ["/docs/", "#install", "https://github.com/RealZST/HarnessKit"]) {
        expect(renderInline(`[x](${href})`)).toEqual([{ kind: "link", href, parts: [text("x")] }]);
      }
    });
    it("throws on any other href, so a bad link fails the build", () => {
      for (const href of ["http://example.com", "javascript:alert(1)", "mailto:a@b.c", "docs/page", "//evil.example"]) {
        expect(() => renderInline(`[x](${href})`), href).toThrow(/href/);
      }
    });
    it("ends the href at the matching paren, allowing one nested pair", () => {
      expect(renderInline("([wiki](https://en.wikipedia.org/wiki/Foo_(bar)))")).toEqual([
        text("("),
        { kind: "link", href: "https://en.wikipedia.org/wiki/Foo_(bar)", parts: [text("wiki")] },
        text(")"),
      ]);
    });
    it("links only the innermost bracket pair: [a [b](c) links b", () => {
      expect(renderInline("[a [b](/c)")).toEqual([text("[a "), { kind: "link", href: "/c", parts: [text("b")] }]);
    });
    it("keeps a backtick span that contains link syntax as code", () => {
      expect(renderInline("`[x](/y)`")).toEqual([code("[x](/y)")]);
    });
  });
});
