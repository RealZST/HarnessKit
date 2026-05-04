import type { HkError, HkErrorKind } from "./error-types";
import { parseError } from "./error-types";

/**
 * Converts a raw backend error into a user-friendly message.
 *
 * Accepts any error shape — strings (HTTP transport), HkError objects (Tauri
 * IPC reject value), Error instances, or anything else. Routes everything
 * through `parseError` so callers don't have to wrap errors in `String()`
 * (which produces `"[object Object]"` for Tauri's reified HkError objects).
 */
export function humanizeError(raw: unknown): string {
  const err: HkError = parseError(raw);

  // Kind-based routing for typed errors
  const kindMessage = humanizeByKind(err.kind, err.message);
  if (kindMessage) return kindMessage;

  // Fallback: string heuristic matching on the message
  return humanizeByMessage(err.message);
}

function humanizeByKind(kind: HkErrorKind, message: string): string | null {
  switch (kind) {
    case "Network":
      return "Could not connect. Check your internet connection and try again.";
    case "NotFound":
      return `Not found: ${truncate(message, 100)}`;
    case "PermissionDenied":
      // Pass through verbatim. PermissionDenied covers both filesystem perms
      // (io::Error) and HK web token auth — the previous git-flavored
      // "repository may be private" message was misleading in both cases.
      return truncate(message, 120);
    case "PathNotAllowed":
      // Preserve the specific reason (e.g. "Config paths cannot contain '..'
      // components") rather than collapsing every PathNotAllowed to a generic
      // sentence — different backend call sites encode different reasons.
      return truncate(message, 120);
    case "ConfigCorrupted":
      return "A configuration file appears to be corrupted. Try resetting it.";
    case "Database":
      return "A database error occurred. Try restarting the app.";
    case "CommandFailed":
      return `Command failed: ${truncate(message, 100)}`;
    case "Conflict":
      return truncate(message, 120);
    case "Validation":
      return truncate(message, 120);
    case "Internal":
      // Fall through to heuristic matching for Internal errors
      return null;
    default:
      return null;
  }
}

function humanizeByMessage(raw: string): string {
  const lower = raw.toLowerCase();

  if (
    lower.includes("network") ||
    lower.includes("fetch") ||
    lower.includes("dns") ||
    lower.includes("econnrefused")
  ) {
    return "Could not connect. Check your internet connection and try again.";
  }

  if (
    lower.includes("git clone") ||
    lower.includes("repository not found") ||
    lower.includes("fatal: repository")
  ) {
    return "Could not access the repository. Check the URL and make sure it's publicly accessible.";
  }

  // HTTP-style auth markers — typically come from git over https. Plain
  // "permission denied" intentionally falls through to the default truncate
  // since it's usually a filesystem-perm error, not a credentials issue.
  if (
    lower.includes("403") ||
    lower.includes("401") ||
    lower.includes("authentication")
  ) {
    return "Access denied. The repository may be private or require authentication.";
  }

  if (
    lower.includes("not found") ||
    lower.includes("404") ||
    lower.includes("no such")
  ) {
    return "Not found. Check the URL or skill ID and try again.";
  }

  if (lower.includes("timeout") || lower.includes("timed out")) {
    return "The request timed out. Try again in a moment.";
  }

  if (
    lower.includes("no space") ||
    lower.includes("disk full") ||
    lower.includes("enospc")
  ) {
    return "Not enough disk space. Free up some space and try again.";
  }

  if (lower.includes("already exists") || lower.includes("duplicate")) {
    return "This extension is already installed.";
  }

  return truncate(raw, 120);
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return `${s.slice(0, max - 3)}...`;
}
