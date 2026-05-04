import { create } from "zustand";
import { cleanChangelog, DISMISS_KEY_PREFIX } from "./update-store";

const RELEASES_URL =
  "https://api.github.com/repos/RealZST/HarnessKit/releases/latest";
const CACHE_KEY = "hk-web-update-cache";
const CACHE_TTL_MS = 60 * 60 * 1000;
const MIN_UPDATE_CHECK_VISIBLE_MS = 600;

interface CachedRelease {
  tag: string;
  body: string;
  at: number;
}

function parseVersion(raw: string): [number, number, number] | null {
  const match = raw.trim().match(/^v?(\d+)\.(\d+)\.(\d+)/);
  if (!match) return null;
  return [Number(match[1]), Number(match[2]), Number(match[3])];
}

function isNewer(current: string, latest: string): boolean {
  const a = parseVersion(current);
  const b = parseVersion(latest);
  if (!a || !b) return false;
  for (let i = 0; i < 3; i++) {
    if (b[i] > a[i]) return true;
    if (b[i] < a[i]) return false;
  }
  return false;
}

function readCache(): CachedRelease | null {
  try {
    const raw = sessionStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const cached = JSON.parse(raw) as CachedRelease;
    if (Date.now() - cached.at > CACHE_TTL_MS) return null;
    return cached;
  } catch {
    return null;
  }
}

function writeCache(tag: string, body: string): void {
  try {
    const entry: CachedRelease = { tag, body, at: Date.now() };
    sessionStorage.setItem(CACHE_KEY, JSON.stringify(entry));
  } catch {
    // sessionStorage unavailable — ignore
  }
}

async function fetchLatestRelease(
  force = false,
): Promise<{ tag: string; body: string } | null> {
  if (!force) {
    const cached = readCache();
    if (cached) return { tag: cached.tag, body: cached.body };
  }
  try {
    const response = await fetch(RELEASES_URL);
    if (!response.ok) return null;
    const data = (await response.json()) as {
      tag_name?: string;
      body?: string;
    };
    if (!data.tag_name) return null;
    const body = data.body ?? "";
    writeCache(data.tag_name, body);
    return { tag: data.tag_name, body };
  } catch {
    return null;
  }
}

interface WebUpdateState {
  /** Available update payload, null if none or not yet checked. */
  available: { version: string; body: string } | null;
  checking: boolean;
  /** Whether the changelog/instructions dialog is visible. */
  showDialog: boolean;
  /** User dismissed the reminder for `available.version` (sidebar card hidden). */
  dismissed: boolean;

  /** When `force` is true, skip the sessionStorage cache and re-fetch from GitHub. */
  checkForUpdate: (force?: boolean) => Promise<void>;
  /** Open the dialog (only when an update is available). */
  promptUpdate: () => void;
  /** Close the dialog without persisting dismissal (X / backdrop). */
  dismissDialog: () => void;
  /** Close the dialog AND persist a "don't remind for this version" flag (Close button). */
  dismissUpdate: () => void;
}

export const useWebUpdateStore = create<WebUpdateState>((set, get) => ({
  available: null,
  checking: false,
  showDialog: false,
  dismissed: false,

  async checkForUpdate(force = false) {
    if (get().checking) return;
    const startedAt = Date.now();
    set({ checking: true });
    try {
      const release = await fetchLatestRelease(force);
      if (!release) return;
      if (!isNewer(__APP_VERSION__, release.tag)) return;
      const version = release.tag.replace(/^v/, "");
      const dismissed =
        localStorage.getItem(`${DISMISS_KEY_PREFIX}${version}`) === "1";
      set({
        available: { version, body: cleanChangelog(release.body) },
        dismissed,
      });
    } finally {
      const remaining = MIN_UPDATE_CHECK_VISIBLE_MS - (Date.now() - startedAt);
      if (remaining > 0) {
        await new Promise((resolve) => setTimeout(resolve, remaining));
      }
      set({ checking: false });
    }
  },

  promptUpdate() {
    if (get().available) {
      set({ showDialog: true });
    }
  },

  dismissDialog() {
    set({ showDialog: false });
  },

  dismissUpdate() {
    const { available } = get();
    if (!available) return;
    try {
      localStorage.setItem(`${DISMISS_KEY_PREFIX}${available.version}`, "1");
    } catch {
      // localStorage unavailable — keep the in-memory dismissal anyway
    }
    set({ showDialog: false, dismissed: true });
  },
}));
