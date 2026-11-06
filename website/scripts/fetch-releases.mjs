// Build-time: fetch the latest GitHub release's desktop installers into src/data/downloads.json and the star count
// into src/data/repo.json. Offline or rate-limited → keep the committed files and exit 0.
import { realpathSync } from "node:fs";
import { basename } from "node:path";
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { releaseDownloads } from "../src/lib/release-assets.mjs";

const DOWNLOADS_OUT = new URL("../src/data/downloads.json", import.meta.url);
const REPO_OUT = new URL("../src/data/repo.json", import.meta.url);
const REPO_API = "https://api.github.com/repos/RealZST/HarnessKit";
// Ask for 30 so drafts and prereleases among the newest can be skipped.
const RELEASES_API = `${REPO_API}/releases?per_page=30`;
const published = (releases) => releases.filter((r) => !r.draft && !r.prerelease);

// The latest published release's installers, picked as the page picks them when it asks GitHub again on load.
export function desktopDownloads(releases) {
  const [latest] = published(releases);
  if (!latest) throw new Error("no releases");
  return releaseDownloads(latest);
}

// Only the count: the committed file then changes only when the star count does.
export function toRepoInfo(repo) {
  if (typeof repo?.stargazers_count !== "number") throw new Error("no stargazers_count in response");
  return { stars: repo.stargazers_count };
}

function requestHeaders() {
  const token = process.env.GITHUB_TOKEN;
  return {
    Accept: "application/vnd.github+json",
    "User-Agent": "harnesskit-website",
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
  };
}

async function getJson(url) {
  const res = await fetch(url, { signal: AbortSignal.timeout(10_000), headers: requestHeaders() });
  if (!res.ok) throw new Error(`GitHub API ${res.status}`);
  return res.json();
}

const fetchDownloads = async () => desktopDownloads(await getJson(RELEASES_API));

const fetchRepoInfo = async () => toRepoInfo(await getJson(REPO_API));

// Missing cache: nothing to keep, exit 0. Malformed cache: the build cannot use it, exit 1.
async function keepCache(file, reason, describe) {
  const name = basename(fileURLToPath(file));
  let raw;
  try {
    raw = await readFile(file, "utf8");
  } catch {
    console.warn(`fetch-releases: ${name}: no cache available`);
    return;
  }
  try {
    console.warn(`fetch-releases: ${name}: ${reason}; keeping ${describe(JSON.parse(raw))}`);
  } catch (parseError) {
    console.error(`fetch-releases: ${name}: ${reason}; cached file is malformed: ${parseError.message}`);
    process.exitCode = 1;
  }
}

// Writes fresh data to `file`, or keeps the committed copy when GitHub can't be reached.
async function refreshCache(file, fetchFresh, describe) {
  try {
    const data = await fetchFresh();
    await writeFile(file, JSON.stringify(data, null, 2) + "\n");
    console.log(`${basename(fileURLToPath(file))}: ${describe(data)}`);
  } catch (err) {
    await keepCache(file, err.message, describe);
  }
}

const describeDownloads = ({ tag, ...assets }) =>
  `${tag}: ${Object.entries(assets).map(([key, asset]) => `${key} ${asset?.name ?? "missing, links the release page"}`).join(", ")}`;

async function main() {
  await Promise.all([
    refreshCache(DOWNLOADS_OUT, fetchDownloads, describeDownloads),
    refreshCache(REPO_OUT, fetchRepoInfo, (info) => `${info.stars} stars`),
  ]);
}

if (process.argv[1] && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
