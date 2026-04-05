export type HkErrorKind =
  | "NotFound"
  | "Network"
  | "PermissionDenied"
  | "ConfigCorrupted"
  | "Conflict"
  | "PathNotAllowed"
  | "Database"
  | "CommandFailed"
  | "Validation"
  | "Internal";

export interface HkError {
  kind: HkErrorKind;
  message: string;
}

/**
 * Parse an unknown error into a structured HkError.
 * Handles:
 *   1. Tauri v2 JSON string format: '{"kind":"Network","message":"timeout"}'
 *   2. Already-parsed object format: {kind: "Network", message: "timeout"}
 *   3. Legacy plain string format: "Network error: timeout"
 */
export function parseError(error: unknown): HkError {
  // Tauri v2 sends HkError as a JSON string via IPC — try parsing it first
  if (typeof error === "string") {
    try {
      const parsed = JSON.parse(error);
      if (
        parsed &&
        typeof parsed === "object" &&
        "kind" in parsed &&
        "message" in parsed
      ) {
        return parsed as HkError;
      }
    } catch {
      // Not valid JSON — fall through to legacy string matching
    }

    // Legacy string format heuristics — order matters (more specific first)
    if (error.includes("Database") || error.includes("database")) {
      return { kind: "Database", message: error };
    }
    if (error.includes("not found") || error.includes("Not found")) {
      return { kind: "NotFound", message: error };
    }
    if (error.includes("Permission denied")) {
      return { kind: "PermissionDenied", message: error };
    }
    if (error.includes("not within") || error.includes("Path not allowed")) {
      return { kind: "PathNotAllowed", message: error };
    }
    if (
      error.includes("Network") ||
      error.includes("timeout") ||
      error.includes("Failed to reach")
    ) {
      return { kind: "Network", message: error };
    }
    return { kind: "Internal", message: error };
  }

  // Already-parsed object format (e.g., from direct JS calls)
  if (
    error &&
    typeof error === "object" &&
    "kind" in error &&
    "message" in error
  ) {
    return error as HkError;
  }

  return { kind: "Internal", message: String(error) };
}

/** Whether the error is likely transient and worth retrying */
export function isRetryable(error: HkError): boolean {
  return error.kind === "Network";
}
