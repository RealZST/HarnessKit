import { describe, expect, it } from "vitest";
import { humanizeError } from "../errors";

describe("humanizeError", () => {
  it("detects network errors", () => {
    expect(humanizeError("network error occurred")).toContain(
      "internet connection",
    );
    expect(humanizeError("fetch failed")).toContain("internet connection");
    expect(humanizeError("ECONNREFUSED")).toContain("internet connection");
    expect(humanizeError("DNS lookup failed")).toContain("internet connection");
  });

  it("detects git clone failures", () => {
    expect(humanizeError("git clone failed")).toContain("repository");
    // "Repository not found" is classified as NotFound by parseError, so
    // humanizeByKind returns a NotFound message rather than the git heuristic
    expect(humanizeError("Repository not found")).toContain("Not found");
    expect(humanizeError("fatal: repository gone")).toContain("repository");
  });

  it("detects HTTP auth errors", () => {
    expect(humanizeError("HTTP 403 Forbidden")).toContain("Access denied");
    expect(humanizeError("401 Unauthorized")).toContain("Access denied");
    expect(humanizeError("authentication required")).toContain("Access denied");
  });

  it("passes plain filesystem 'permission denied' through (not git-flavored)", () => {
    // No git-flavored "repository may be private" wording — that misled
    // users when the real cause was filesystem perms or a stale custom path.
    expect(humanizeError("permission denied")).toBe("permission denied");
  });

  it("preserves PathNotAllowed message instead of collapsing to a generic sentence", () => {
    // Different backend call sites encode different reasons — e.g. ".."
    // rejection vs. policy violation. The humanizer used to flatten them all
    // to "This path is not within an allowed directory." which hid the cause.
    expect(
      humanizeError({
        kind: "PathNotAllowed",
        message: "Config paths cannot contain '..' components",
      }),
    ).toBe("Config paths cannot contain '..' components");
  });

  it("accepts Tauri-shaped HkError objects (not just strings)", () => {
    // Tauri IPC rejects with a reified object, not a JSON string. Earlier
    // call sites wrapped errors in `String()` and produced "[object Object]".
    expect(humanizeError({ kind: "Validation", message: "bad input" })).toBe(
      "bad input",
    );
  });

  it("detects not-found errors", () => {
    expect(humanizeError("not found")).toContain("Not found");
    expect(humanizeError("404")).toContain("Not found");
    expect(humanizeError("no such file")).toContain("Not found");
  });

  it("detects timeout errors", () => {
    // "request timeout" contains "timeout" -> classified as Network by parseError
    expect(humanizeError("request timeout")).toContain("internet connection");
    // "timed out" does NOT contain "timeout" -> classified as Internal,
    // then falls through to humanizeByMessage which matches "timed out"
    expect(humanizeError("timed out after 30s")).toContain("timed out");
  });

  it("detects disk space errors", () => {
    expect(humanizeError("no space left")).toContain("disk space");
    expect(humanizeError("ENOSPC")).toContain("disk space");
    expect(humanizeError("disk full")).toContain("disk space");
  });

  it("detects duplicate/already-exists errors", () => {
    expect(humanizeError("already exists")).toContain("already installed");
    expect(humanizeError("duplicate entry")).toContain("already installed");
  });

  it("returns short messages as-is", () => {
    expect(humanizeError("something weird")).toBe("something weird");
  });

  it("truncates long messages", () => {
    const long = "x".repeat(200);
    const result = humanizeError(long);
    expect(result.length).toBeLessThanOrEqual(120);
    expect(result).toMatch(/\.\.\.$/);
  });
});
