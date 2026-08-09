import i18n from "@/lib/i18n";
import { toast } from "@/stores/toast-store";

/** Copy a filesystem path to the clipboard, toasting success or failure.
 *  Web mode's stand-in for "open in Finder / open in editor" affordances. */
export async function copyPathToClipboard(path: string) {
  try {
    await navigator.clipboard.writeText(path);
    toast.success(i18n.t("agents:toast.pathCopied"));
  } catch {
    toast.error(i18n.t("agents:toast.failedCopyPath"));
  }
}
