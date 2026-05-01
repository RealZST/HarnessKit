import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { create } from "zustand";

/** Clean up GitHub auto-generated release notes for in-app display.
 *  - Removes "New Contributors" and "Full Changelog" sections
 *  - Converts bare PR URLs to clickable markdown links (e.g. [#3](url)) */
export function cleanChangelog(body: string): string {
  const lines: string[] = [];
  let skip = false;
  for (const line of body.split("\n")) {
    // Skip "New Contributors" section and everything after "Full Changelog"
    if (
      line.startsWith("## New Contributors") ||
      line.startsWith("**Full Changelog**")
    ) {
      skip = true;
      continue;
    }
    // Resume after skipped section ends with a new heading
    if (skip && line.startsWith("## ")) {
      skip = false;
    }
    if (skip) continue;
    // Convert "in https://...github.com/.../pull/N" to "in [#N](url)"
    const converted = line.replace(
      /in (https:\/\/github\.com\/[^\s]+\/pull\/(\d+))/g,
      (_match, url, num) => `in [#${num}](${url})`,
    );
    lines.push(converted);
  }
  return lines.join("\n").trim();
}

/** localStorage key prefix for "user has dismissed the update reminder for this version".
 *  Shared between desktop and web update flows so a dismissal in either mode silences both. */
export const DISMISS_KEY_PREFIX = "hk-update-dismissed-v";

interface UpdateState {
  /** Available update version, null if none or not checked yet */
  available: { version: string; body: string } | null;
  checking: boolean;
  installing: boolean;
  /** Whether the changelog confirmation dialog is visible */
  showChangelog: boolean;
  /** User dismissed the reminder for `available.version` (sidebar card hidden). */
  dismissed: boolean;

  checkForUpdate: () => Promise<void>;
  /** Open the changelog dialog (called when user clicks Update) */
  promptUpdate: () => void;
  /** Close the changelog dialog without updating (X / backdrop). Does not hide card. */
  dismissDialog: () => void;
  /** Close the dialog AND persist a "don't remind for this version" flag (Later button). */
  dismissUpdate: () => void;
  /** Confirm and install the update */
  confirmUpdate: () => Promise<void>;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  available: null,
  checking: false,
  installing: false,
  showChangelog: false,
  dismissed: false,

  async checkForUpdate() {
    if (get().checking) return;
    set({ checking: true });
    try {
      const update = await check();
      if (update) {
        const dismissed =
          localStorage.getItem(`${DISMISS_KEY_PREFIX}${update.version}`) ===
          "1";
        set({
          available: {
            version: update.version,
            body: cleanChangelog(update.body ?? ""),
          },
          dismissed,
        });
      }
    } catch {
      // Silent failure — update check is non-critical
    } finally {
      set({ checking: false });
    }
  },

  promptUpdate() {
    if (get().available) {
      set({ showChangelog: true });
    }
  },

  dismissDialog() {
    set({ showChangelog: false });
  },

  dismissUpdate() {
    const { available } = get();
    if (!available) return;
    try {
      localStorage.setItem(`${DISMISS_KEY_PREFIX}${available.version}`, "1");
    } catch {
      // localStorage unavailable — keep the in-memory dismissal anyway
    }
    set({ showChangelog: false, dismissed: true });
  },

  async confirmUpdate() {
    if (get().installing) return;
    set({ installing: true, showChangelog: false });
    try {
      const update = await check();
      if (update) {
        await update.downloadAndInstall();
        await relaunch();
      }
    } catch {
      set({ installing: false });
    }
  },
}));
