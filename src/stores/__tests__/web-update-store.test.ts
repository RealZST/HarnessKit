import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const RELEASES_URL =
  "https://api.github.com/repos/RealZST/HarnessKit/releases/latest";
const CACHE_KEY = "hk-web-update-cache";
const DISMISS_KEY_PREFIX = "hk-update-dismissed-v";

function mockReleasesResponse(tag: string, body = "") {
  return {
    ok: true,
    json: async () => ({ tag_name: tag, body }),
  } as Response;
}

describe("web-update-store", () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    vi.resetModules();
    vi.unstubAllGlobals();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("starts with no update available", async () => {
    const { useWebUpdateStore } = await import("../web-update-store");
    const state = useWebUpdateStore.getState();
    expect(state.available).toBeNull();
    expect(state.checking).toBe(false);
    expect(state.showDialog).toBe(false);
    expect(state.dismissed).toBe(false);
  });

  it("does not flag update when current version matches latest", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockReleasesResponse("v1.2.1")),
    );

    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();

    expect(useWebUpdateStore.getState().available).toBeNull();
  });

  it("flags update when latest is newer than current", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockReleasesResponse("v1.3.0", "## Changes")),
    );

    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();

    const { available } = useWebUpdateStore.getState();
    expect(available?.version).toBe("1.3.0");
    expect(available?.body).toContain("## Changes");
  });

  it("does not flag update when latest is older (downgrade)", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockReleasesResponse("v1.0.0")),
    );

    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();

    expect(useWebUpdateStore.getState().available).toBeNull();
  });

  it("strips 'New Contributors' and 'Full Changelog' sections from body", async () => {
    const raw = [
      "## What's Changed",
      "- feat: thing",
      "## New Contributors",
      "@someone made their first contribution",
      "**Full Changelog**: https://...",
    ].join("\n");
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockReleasesResponse("v1.3.0", raw)),
    );

    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();

    const body = useWebUpdateStore.getState().available?.body ?? "";
    expect(body).toContain("## What's Changed");
    expect(body).toContain("- feat: thing");
    expect(body).not.toContain("New Contributors");
    expect(body).not.toContain("Full Changelog");
  });

  it("promptUpdate opens dialog only when an update is available", async () => {
    const { useWebUpdateStore } = await import("../web-update-store");

    useWebUpdateStore.getState().promptUpdate();
    expect(useWebUpdateStore.getState().showDialog).toBe(false);

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockReleasesResponse("v1.3.0")),
    );
    await useWebUpdateStore.getState().checkForUpdate();
    useWebUpdateStore.getState().promptUpdate();
    expect(useWebUpdateStore.getState().showDialog).toBe(true);
  });

  it("dismissDialog closes the dialog without persisting dismissal", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockReleasesResponse("v1.3.0")),
    );
    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();
    useWebUpdateStore.getState().promptUpdate();
    expect(useWebUpdateStore.getState().showDialog).toBe(true);

    useWebUpdateStore.getState().dismissDialog();
    expect(useWebUpdateStore.getState().showDialog).toBe(false);
    expect(useWebUpdateStore.getState().dismissed).toBe(false);
    expect(localStorage.getItem(`${DISMISS_KEY_PREFIX}1.3.0`)).toBeNull();
  });

  it("dismissUpdate closes the dialog AND persists dismissal for the version", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockReleasesResponse("v1.3.0")),
    );
    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();
    useWebUpdateStore.getState().promptUpdate();

    useWebUpdateStore.getState().dismissUpdate();
    expect(useWebUpdateStore.getState().showDialog).toBe(false);
    expect(useWebUpdateStore.getState().dismissed).toBe(true);
    expect(localStorage.getItem(`${DISMISS_KEY_PREFIX}1.3.0`)).toBe("1");
  });

  it("dismissUpdate is a no-op when no update is available", async () => {
    const { useWebUpdateStore } = await import("../web-update-store");
    useWebUpdateStore.getState().dismissUpdate();
    expect(useWebUpdateStore.getState().dismissed).toBe(false);
    expect(localStorage.length).toBe(0);
  });

  it("checkForUpdate seeds dismissed=true when localStorage has flag for the detected version", async () => {
    localStorage.setItem(`${DISMISS_KEY_PREFIX}1.3.0`, "1");
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockReleasesResponse("v1.3.0")),
    );
    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();

    expect(useWebUpdateStore.getState().available?.version).toBe("1.3.0");
    expect(useWebUpdateStore.getState().dismissed).toBe(true);
  });

  it("checkForUpdate keeps dismissed=false when a newer version appears", async () => {
    localStorage.setItem(`${DISMISS_KEY_PREFIX}1.3.0`, "1");
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockReleasesResponse("v1.4.0")),
    );
    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();

    expect(useWebUpdateStore.getState().available?.version).toBe("1.4.0");
    expect(useWebUpdateStore.getState().dismissed).toBe(false);
  });

  it("uses sessionStorage cache instead of refetching within TTL", async () => {
    sessionStorage.setItem(
      CACHE_KEY,
      JSON.stringify({ tag: "v1.3.0", body: "cached body", at: Date.now() }),
    );
    const fetchSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);

    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();

    expect(fetchSpy).not.toHaveBeenCalled();
    expect(useWebUpdateStore.getState().available?.version).toBe("1.3.0");
    expect(useWebUpdateStore.getState().available?.body).toContain(
      "cached body",
    );
  });

  it("ignores stale sessionStorage cache and refetches", async () => {
    sessionStorage.setItem(
      CACHE_KEY,
      JSON.stringify({
        tag: "v1.3.0",
        body: "old",
        at: Date.now() - 2 * 60 * 60 * 1000,
      }),
    );
    const fetchSpy = vi
      .fn()
      .mockResolvedValue(mockReleasesResponse("v1.4.0", "fresh"));
    vi.stubGlobal("fetch", fetchSpy);

    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();

    expect(fetchSpy).toHaveBeenCalledWith(RELEASES_URL);
    expect(useWebUpdateStore.getState().available?.version).toBe("1.4.0");
    expect(useWebUpdateStore.getState().available?.body).toContain("fresh");
  });

  it("silently absorbs fetch failures", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("network")));

    const { useWebUpdateStore } = await import("../web-update-store");
    await expect(
      useWebUpdateStore.getState().checkForUpdate(),
    ).resolves.toBeUndefined();
    expect(useWebUpdateStore.getState().available).toBeNull();
  });

  it("silently absorbs non-OK HTTP responses", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: false, status: 500 } as Response),
    );

    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();
    expect(useWebUpdateStore.getState().available).toBeNull();
  });

  it("ignores malformed tag_name", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ tag_name: "garbage" }),
      } as Response),
    );

    const { useWebUpdateStore } = await import("../web-update-store");
    await useWebUpdateStore.getState().checkForUpdate();
    expect(useWebUpdateStore.getState().available).toBeNull();
  });
});
