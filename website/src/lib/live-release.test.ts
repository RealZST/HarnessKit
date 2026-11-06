import { afterEach, describe, expect, it, vi } from "vitest";
import { applyLive, fetchLive, refreshLive } from "./live-release";

// The page's download links and the nav's count as the build renders them.
function page() {
  const root = document.createElement("div");
  root.innerHTML = `
    <a data-asset="macArm64" href="https://github.com/o/r/releases/download/v1.15.1/HarnessKit_1.15.1_aarch64.dmg">mac</a>
    <a data-asset="macX64" href="https://github.com/o/r/releases/download/v1.15.1/HarnessKit_1.15.1_x64.dmg">intel</a>
    <a data-asset="windowsX64" href="https://github.com/o/r/releases/download/v1.15.1/HarnessKit_1.15.1_x64-setup.exe">win</a>
    <span data-stars>★ 482</span>`;
  return root;
}
const hrefs = (root: HTMLElement) => [...root.querySelectorAll("a")].map((a) => a.getAttribute("href"));

const asset = (name: string) => ({ name, browser_download_url: `https://github.com/o/r/releases/download/v1.16.0/${name}` });
const v1160 = { tag_name: "v1.16.0", assets: [asset("HarnessKit_1.16.0_aarch64.dmg"), asset("HarnessKit_1.16.0_x64.dmg"), asset("HarnessKit_1.16.0_x64-setup.exe"), asset("hk-windows-x64.exe")] };
const json = (body: unknown, ok = true) => Promise.resolve({ ok, status: ok ? 200 : 403, json: () => Promise.resolve(body) } as Response);
// GitHub as the page sees it: the latest release, and the repository with its star count.
const github = (release: unknown, repo: unknown) =>
  vi.fn((url: string) => (url.endsWith("/releases/latest") ? release : repo) as Promise<Response>) as unknown as typeof fetch;

afterEach(() => {
  sessionStorage.clear();
  vi.useRealTimers();
});

describe("the latest release on the page", () => {
  it("moves each download link to the latest release's installer and updates the star count", async () => {
    const root = page();
    await refreshLive(root, github(json(v1160), json({ stargazers_count: 1234 })));
    expect(hrefs(root)).toEqual(v1160.assets.slice(0, 3).map((a) => a.browser_download_url));
    expect(root.querySelector("[data-stars]")?.textContent).toBe("★ 1.2K");
  });

  it("keeps the build's links and count where GitHub doesn't answer", async () => {
    const root = page();
    const before = hrefs(root);
    await refreshLive(root, vi.fn(() => Promise.reject(new TypeError("offline"))) as unknown as typeof fetch);
    expect(hrefs(root)).toEqual(before);
    expect(root.querySelector("[data-stars]")?.textContent).toBe("★ 482");
  });

  it("uses whichever answer came back when the other request fails", async () => {
    const root = page();
    const before = hrefs(root);
    await refreshLive(root, github(json({ message: "rate limited" }, false), json({ stargazers_count: 500 })));
    expect(hrefs(root)).toEqual(before);
    expect(root.querySelector("[data-stars]")?.textContent).toBe("★ 500");
  });

  it("keeps the build's link for a platform the latest release has no installer for", () => {
    const root = page();
    applyLive(root, { downloads: { macArm64: "https://example.test/new.dmg", macX64: null, windowsX64: null } });
    expect(hrefs(root)).toEqual(["https://example.test/new.dmg", ...hrefs(page()).slice(1)]);
  });

  it("asks GitHub once a visit: the next page takes the answer it kept", async () => {
    const fetchFn = github(json(v1160), json({ stargazers_count: 600 }));
    await refreshLive(page(), fetchFn);
    const next = page();
    await refreshLive(next, fetchFn);
    expect(fetchFn).toHaveBeenCalledTimes(2);
    expect(next.querySelector("[data-stars]")?.textContent).toBe("★ 600");
    expect(hrefs(next)[0]).toBe(v1160.assets[0].browser_download_url);
  });

  it("asks again once the kept answer is half an hour old", async () => {
    vi.useFakeTimers();
    const fetchFn = github(json(v1160), json({ stargazers_count: 600 }));
    await refreshLive(page(), fetchFn);
    vi.advanceTimersByTime(31 * 60 * 1000);
    await refreshLive(page(), fetchFn);
    expect(fetchFn).toHaveBeenCalledTimes(4);
  });

  it("reads the installers with the build's rules: never the CLI binary", async () => {
    const live = await fetchLive(github(json(v1160), json({})));
    expect(live.downloads?.windowsX64).toMatch(/x64-setup\.exe$/);
    expect(live.stars).toBeUndefined();
  });
});
