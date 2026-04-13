import { describe, expect, it } from "vitest";
import { isRetryable, parseError } from "../error-types";

describe("parseError", () => {
  it("parses Tauri v2 JSON string format", () => {
    const err = parseError('{"kind":"Network","message":"timeout"}');
    expect(err.kind).toBe("Network");
    expect(err.message).toBe("timeout");
  });

  it("parses Tauri v2 JSON string with nested quotes", () => {
    const err = parseError(
      '{"kind":"CommandFailed","message":"git clone failed: permission denied"}',
    );
    expect(err.kind).toBe("CommandFailed");
    expect(err.message).toContain("git clone failed");
  });

  it("handles already-parsed object format", () => {
    const err = parseError({ kind: "NotFound", message: "skill missing" });
    expect(err.kind).toBe("NotFound");
    expect(err.message).toBe("skill missing");
  });

  it("classifies legacy plain string with 'not found'", () => {
    const err = parseError("Extension not found");
    expect(err.kind).toBe("NotFound");
    expect(err.message).toBe("Extension not found");
  });

  it("classifies legacy plain string with 'Network'", () => {
    const err = parseError("Network error: connection refused");
    expect(err.kind).toBe("Network");
  });

  it("classifies legacy plain string with 'Permission denied'", () => {
    const err = parseError("Permission denied: /etc/shadow");
    expect(err.kind).toBe("PermissionDenied");
  });

  it("classifies legacy plain string with 'Database'", () => {
    const err = parseError("Database error: table not found");
    expect(err.kind).toBe("Database");
  });

  it("falls back to Internal for unknown strings", () => {
    const err = parseError("something went wrong");
    expect(err.kind).toBe("Internal");
    expect(err.message).toBe("something went wrong");
  });

  it("handles non-string non-object inputs", () => {
    const err = parseError(42);
    expect(err.kind).toBe("Internal");
    expect(err.message).toBe("42");
  });

  it("handles null/undefined", () => {
    expect(parseError(null).kind).toBe("Internal");
    expect(parseError(undefined).kind).toBe("Internal");
  });

  it("does not treat valid JSON without kind/message as HkError", () => {
    const err = parseError('{"error":"something"}');
    // Should fall through to legacy string matching since no kind/message
    expect(err.kind).toBe("Internal");
  });
});

describe("isRetryable", () => {
  it("returns true for Network errors", () => {
    expect(isRetryable({ kind: "Network", message: "timeout" })).toBe(true);
  });

  it("returns false for non-Network errors", () => {
    expect(isRetryable({ kind: "NotFound", message: "x" })).toBe(false);
    expect(isRetryable({ kind: "Internal", message: "x" })).toBe(false);
    expect(isRetryable({ kind: "Database", message: "x" })).toBe(false);
  });
});
