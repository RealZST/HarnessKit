// Which file of a GitHub release is which platform's desktop installer. Shared by the build (scripts/fetch-releases.mjs,
// which writes src/data/downloads.json) and the page (src/lib/live-release.ts, which asks GitHub again on load), so the
// two always pick the same files. Plain JavaScript: the build script runs in Node as is.

// The desktop installers, by platform and CPU. tauri-action names them HarnessKit_1.15.1_aarch64.dmg,
// HarnessKit_1.15.1_x64.dmg and HarnessKit_1.15.1_x64-setup.exe (the App's .github/workflows/release.yml); the
// patterns also take the other spellings of each CPU, a universal DMG and an .msi. The hk-* CLI binaries
// (hk-windows-x64.exe), the updater bundles (.app.tar.gz) and the .sig files never match.
const DESKTOP_ASSETS = {
  macArm64: (name) => /\.dmg$/i.test(name) && /aarch64|arm64|universal/i.test(name),
  macX64: (name) => /\.dmg$/i.test(name) && /x64|x86[_-]64|universal/i.test(name),
  windowsX64: (name) => /(-setup\.exe|\.msi)$/i.test(name) && /x64|x86[_-]64/i.test(name),
};

/** The platforms, as downloads.json and the pages' data-asset attributes name them. */
export const PLATFORMS = /** @type {const} */ (["macArm64", "macX64", "windowsX64"]);

/**
 * One download per platform from a release: its file name and URL, or null when the release has none (the site then
 * links the release page). An .exe setup wins over an .msi: it is the one the README names.
 * @param {{ tag_name: string, assets: { name: string, browser_download_url: string }[] }} release
 * @returns {{ tag: string, macArm64: Asset | null, macX64: Asset | null, windowsX64: Asset | null }}
 * @typedef {{ name: string, url: string }} Asset
 */
export function releaseDownloads(release) {
  const assets = [...release.assets].sort((a, b) => Number(/\.msi$/i.test(a.name)) - Number(/\.msi$/i.test(b.name)));
  /** @param {(name: string) => boolean} matches */
  const pick = (matches) => {
    const asset = assets.find((a) => matches(a.name));
    return asset ? { name: asset.name, url: asset.browser_download_url } : null;
  };
  return {
    tag: release.tag_name,
    macArm64: pick(DESKTOP_ASSETS.macArm64),
    macX64: pick(DESKTOP_ASSETS.macX64),
    windowsX64: pick(DESKTOP_ASSETS.windowsX64),
  };
}
