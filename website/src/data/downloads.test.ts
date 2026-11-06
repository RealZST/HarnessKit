import { describe, expect, it } from "vitest";
import { DOWNLOADS, downloadLinks } from "./downloads";
import { RELEASES_URL } from "./agents";

describe("downloadLinks", () => {
  it("links each installer directly", () => {
    const asset = { name: "HarnessKit_1.15.1_x64.dmg", url: "https://example.test/x64.dmg" };
    expect(downloadLinks({ tag: "v1.15.1", macArm64: asset, macX64: asset, windowsX64: asset }).macX64).toEqual({ href: asset.url, direct: true });
  });
  it("falls back to the latest release page for an installer the release doesn't have", () => {
    expect(downloadLinks({ tag: "v1.14.0", macArm64: null, macX64: null, windowsX64: null }).windowsX64).toEqual({ href: `${RELEASES_URL}/latest`, direct: false });
  });
});

describe("the committed downloads.json", () => {
  it("links GitHub release assets or the release page", () => {
    for (const link of Object.values(DOWNLOADS)) expect(link.href).toMatch(new RegExp(`^${RELEASES_URL}/`));
  });
});
