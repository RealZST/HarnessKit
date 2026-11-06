import downloads from "./downloads.json";
import { RELEASES_URL } from "./agents";

interface Asset { name: string; url: string }
export interface Downloads { tag: string; macArm64: Asset | null; macX64: Asset | null; windowsX64: Asset | null }
export interface DownloadLink { href: string; direct: boolean }

// Refreshed at build time by scripts/fetch-releases.mjs; the committed file is the offline fallback.
const RELEASE_PAGE = `${RELEASES_URL}/latest`;

// The installer itself where the latest release has one; otherwise the release page, where the visitor picks. `direct`
// says which: a direct download doesn't leave the page, so its text link carries no ↗.
const linkTo = (asset: Asset | null): DownloadLink => (asset ? { href: asset.url, direct: true } : { href: RELEASE_PAGE, direct: false });

export function downloadLinks(data: Downloads) {
  return { macArm64: linkTo(data.macArm64), macX64: linkTo(data.macX64), windowsX64: linkTo(data.windowsX64) };
}

export const DOWNLOADS = downloadLinks(downloads as Downloads);
