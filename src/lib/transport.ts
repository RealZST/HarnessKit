/**
 * Transport layer abstraction.
 * Detects whether we're running inside Tauri (desktop) or a plain browser (web mode).
 * In Tauri: uses IPC invoke(). In browser: uses HTTP POST to /api/{command}.
 */

// Tauri v2 injects __TAURI_INTERNALS__ on the window object
const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// Use a Promise to avoid race condition: the first API call waits for the
// dynamic import to resolve before proceeding.
const tauriInvokePromise: Promise<
  (cmd: string, args?: Record<string, unknown>) => Promise<unknown>
> | null = isTauri
  ? import("@tauri-apps/api/core").then((mod) => mod.invoke)
  : null;

/**
 * Call a backend command.
 * - In Tauri: `invoke(command, args)` via IPC
 * - In browser: `POST /api/{command}` with JSON body
 */
export async function transport<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (tauriInvokePromise) {
    const invoke = await tauriInvokePromise;
    return invoke(command, args) as Promise<T>;
  }
  return httpInvoke<T>(command, args);
}

/** Token for authenticated web mode — set by the login page or URL param */
let authToken: string | null = null;

export function setAuthToken(token: string): void {
  authToken = token;
  sessionStorage.setItem("hk_token", token);
}

export function getAuthToken(): string | null {
  if (authToken) return authToken;
  authToken = sessionStorage.getItem("hk_token");
  return authToken;
}

/** Convert camelCase keys to snake_case (Tauri invoke does this automatically) */
function toSnakeKeys(obj: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj)) {
    const snakeKey = key.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
    result[snakeKey] = value;
  }
  return result;
}

async function httpInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  const token = getAuthToken();
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const response = await fetch(`/api/${command}`, {
    method: "POST",
    headers,
    body: JSON.stringify(toSnakeKeys(args ?? {})),
  });

  if (!response.ok) {
    const text = await response.text();
    // Throw as-is — parseError() in error-types.ts handles both
    // JSON HkError strings and plain text formats
    throw text || `HTTP ${response.status}`;
  }

  return response.json() as Promise<T>;
}

/** Whether we're running in Tauri desktop or web browser */
export function isDesktop(): boolean {
  return isTauri;
}
