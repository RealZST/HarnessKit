// The page asks GitHub for the latest release and the star count once it has loaded, so a release reaches the download
// links and the nav without rebuilding the site. The build's values (src/data/downloads.json, repo.json) stay in the
// markup until then, and stay for good where GitHub can't be reached (offline, rate-limited, or slow, as it can be
// from mainland China): every way still downloads, if not always the newest.
import { PLATFORMS, releaseDownloads } from "./release-assets.mjs";
import { formatCount } from "../data/repo";

const API = "https://api.github.com/repos/RealZST/HarnessKit";
// One answer serves a visit: the landing, then the download page, ask once between them.
const CACHE_KEY = "hk:live-release";
const CACHE_MS = 30 * 60 * 1000;
const TIMEOUT_MS = 5000;

type Platform = (typeof PLATFORMS)[number];
export interface Live {
  /** Each platform's installer URL in the latest release, or null where it has none. */
  downloads?: Partial<Record<Platform, string | null>>;
  stars?: number;
}

async function getJson(url: string, fetchFn: typeof fetch): Promise<unknown> {
  const res = await fetchFn(url, { signal: AbortSignal.timeout(TIMEOUT_MS), headers: { Accept: "application/vnd.github+json" } });
  if (!res.ok) throw new Error(`GitHub API ${res.status}`);
  return res.json();
}

/** The latest release's installers and the star count; each left out where its request fails. */
export async function fetchLive(fetchFn: typeof fetch = fetch): Promise<Live> {
  const [release, repo] = await Promise.allSettled([getJson(`${API}/releases/latest`, fetchFn), getJson(API, fetchFn)]);
  const live: Live = {};
  if (release.status === "fulfilled") {
    const picked = releaseDownloads(release.value as Parameters<typeof releaseDownloads>[0]);
    live.downloads = Object.fromEntries(PLATFORMS.map((p) => [p, picked[p]?.url ?? null]));
  }
  if (repo.status === "fulfilled") {
    const stars = (repo.value as { stargazers_count?: unknown }).stargazers_count;
    if (typeof stars === "number") live.stars = stars;
  }
  return live;
}

/**
 * Puts the answer on the page: each download link (a[data-asset], named by platform) to the latest release's file, and
 * the nav's count ([data-stars]). A platform the latest release has no installer for keeps the build's link, which
 * still downloads.
 */
export function applyLive(root: ParentNode, live: Live) {
  for (const link of root.querySelectorAll<HTMLAnchorElement>("a[data-asset]")) {
    const url = live.downloads?.[link.dataset.asset as Platform];
    if (url) link.href = url;
  }
  if (live.stars !== undefined) {
    for (const el of root.querySelectorAll<HTMLElement>("[data-stars]")) el.textContent = `★ ${formatCount(live.stars)}`;
  }
}

function readCache(): Live | undefined {
  try {
    const cached = JSON.parse(sessionStorage.getItem(CACHE_KEY) ?? "null") as { at: number; live: Live } | null;
    return cached && Date.now() - cached.at < CACHE_MS ? cached.live : undefined;
  } catch {
    return undefined;
  }
}

function writeCache(live: Live) {
  try {
    sessionStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), live }));
  } catch {
    // Private windows and blocked storage: the next page asks again.
  }
}

/** Brings the page up to date: from this visit's earlier answer, else from GitHub. Never throws. */
export async function refreshLive(root: ParentNode = document, fetchFn: typeof fetch = fetch): Promise<void> {
  const cached = readCache();
  if (cached) return applyLive(root, cached);
  try {
    const live = await fetchLive(fetchFn);
    if (live.downloads || live.stars !== undefined) writeCache(live);
    applyLive(root, live);
  } catch {
    // The build's values stay.
  }
}
