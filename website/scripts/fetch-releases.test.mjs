import { describe, expect, it } from "vitest";
import { desktopDownloads, toRepoInfo } from "./fetch-releases.mjs";

describe("desktopDownloads", () => {
  const asset = (name) => ({ name, browser_download_url: `https://example.test/${name}` });
  // v1.15.1's assets as the GitHub API listed them.
  const v1151 = [
    "HarnessKit_1.15.1_aarch64.dmg", "HarnessKit_1.15.1_x64-setup.exe", "HarnessKit_1.15.1_x64-setup.exe.sig",
    "HarnessKit_1.15.1_x64.dmg", "HarnessKit_aarch64.app.tar.gz", "HarnessKit_aarch64.app.tar.gz.sig",
    "HarnessKit_x64.app.tar.gz", "HarnessKit_x64.app.tar.gz.sig", "hk-linux-arm64", "hk-linux-x64", "hk-macos-arm64",
    "hk-macos-x64", "hk-windows-x64.exe", "latest.json",
  ].map(asset);
  const rel = (tag, assets, extra = {}) => ({ tag_name: tag, assets, ...extra });

  it("picks each platform's installer from the latest published release, never a CLI binary or an updater bundle", () => {
    expect(desktopDownloads([rel("v1.16.0-rc.1", [], { prerelease: true }), rel("v1.15.1", v1151)])).toEqual({
      tag: "v1.15.1",
      macArm64: { name: "HarnessKit_1.15.1_aarch64.dmg", url: "https://example.test/HarnessKit_1.15.1_aarch64.dmg" },
      macX64: { name: "HarnessKit_1.15.1_x64.dmg", url: "https://example.test/HarnessKit_1.15.1_x64.dmg" },
      windowsX64: { name: "HarnessKit_1.15.1_x64-setup.exe", url: "https://example.test/HarnessKit_1.15.1_x64-setup.exe" },
    });
  });
  it("leaves a platform the release has no installer for as null (v1.14.0 had no Windows build)", () => {
    const v1140 = ["HarnessKit_1.14.0_aarch64.dmg", "HarnessKit_1.14.0_x64.dmg", "hk-windows-x64.exe"].map(asset);
    expect(desktopDownloads([rel("v1.14.0", v1140)]).windowsX64).toBeNull();
  });
  it("takes the other spellings of each CPU, a universal DMG and an .msi, and prefers the setup .exe", () => {
    const names = (assets) => {
      const { tag, ...picked } = desktopDownloads([rel("v2", assets.map(asset))]);
      return Object.fromEntries(Object.entries(picked).map(([key, a]) => [key, a?.name ?? null]));
    };
    expect(names(["App_2_arm64.dmg", "App_2_x86_64.dmg", "App_2_x64_en-US.msi"])).toEqual({ macArm64: "App_2_arm64.dmg", macX64: "App_2_x86_64.dmg", windowsX64: "App_2_x64_en-US.msi" });
    expect(names(["App_2_universal.dmg", "App_2_x64_en-US.msi", "App_2_x64-setup.exe"])).toEqual({ macArm64: "App_2_universal.dmg", macX64: "App_2_universal.dmg", windowsX64: "App_2_x64-setup.exe" });
    expect(names(["App_2_arm64-setup.exe"])).toEqual({ macArm64: null, macX64: null, windowsX64: null });
  });
  it("rejects a list with no published release, so the committed file is kept", () => {
    expect(() => desktopDownloads([rel("v1", v1151, { draft: true })])).toThrow();
  });
});

describe("toRepoInfo", () => {
  it("keeps only the star count, so the cache changes only when the count does", () => {
    expect(toRepoInfo({ stargazers_count: 467, forks_count: 9 })).toEqual({ stars: 467 });
  });
  it("rejects a response without a star count", () => {
    expect(() => toRepoInfo({ message: "Not Found" })).toThrow();
  });
});
