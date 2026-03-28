/**
 * Converts raw backend error strings into user-friendly messages.
 * Pure function, no side effects.
 */
export function humanizeError(raw: string): string {
  const lower = raw.toLowerCase();

  // Network errors
  if (lower.includes("network") || lower.includes("fetch") || lower.includes("dns") || lower.includes("econnrefused")) {
    return "Could not connect. Check your internet connection and try again.";
  }

  // Git clone/fetch failures
  if (lower.includes("git clone") || lower.includes("repository not found") || lower.includes("fatal: repository")) {
    return "Could not access the repository. Check the URL and make sure it's publicly accessible.";
  }

  // Authentication/permission errors
  if (lower.includes("permission denied") || lower.includes("403") || lower.includes("401") || lower.includes("authentication")) {
    return "Access denied. The repository may be private or require authentication.";
  }

  // Not found
  if (lower.includes("not found") || lower.includes("404") || lower.includes("no such")) {
    return "Not found. Check the URL or skill ID and try again.";
  }

  // Timeout
  if (lower.includes("timeout") || lower.includes("timed out")) {
    return "The request timed out. Try again in a moment.";
  }

  // Disk/IO errors
  if (lower.includes("no space") || lower.includes("disk full") || lower.includes("enospc")) {
    return "Not enough disk space. Free up some space and try again.";
  }

  // Already exists
  if (lower.includes("already exists") || lower.includes("duplicate")) {
    return "This extension is already installed.";
  }

  // Fallback: if the message is short enough (< 120 chars), show it as-is.
  // If it's long (likely a stack trace), truncate.
  if (raw.length > 120) {
    return raw.slice(0, 117) + "...";
  }

  return raw;
}
