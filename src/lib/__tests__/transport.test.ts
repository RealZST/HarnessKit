import { beforeEach, describe, expect, it } from "vitest";
import { consumeUrlToken, getAuthToken } from "@/lib/transport";

beforeEach(() => {
  sessionStorage.clear();
  // Reset the URL between tests (jsdom keeps the last replaceState value).
  window.history.replaceState({}, "", "/");
});

describe("consumeUrlToken", () => {
  it("stores the token and strips it from the URL", () => {
    window.history.replaceState({}, "", "/?token=abc123");

    consumeUrlToken();

    expect(getAuthToken()).toBe("abc123");
    expect(sessionStorage.getItem("hk_token")).toBe("abc123");
    expect(window.location.search).toBe("");
  });

  it("preserves other query params while removing only the token", () => {
    window.history.replaceState({}, "", "/?scope=all&token=abc123");

    consumeUrlToken();

    expect(sessionStorage.getItem("hk_token")).toBe("abc123");
    expect(window.location.search).toBe("?scope=all");
  });

  it("is a no-op when no token param is present", () => {
    window.history.replaceState({}, "", "/?scope=all");

    consumeUrlToken();

    expect(sessionStorage.getItem("hk_token")).toBeNull();
    expect(window.location.search).toBe("?scope=all");
  });
});
