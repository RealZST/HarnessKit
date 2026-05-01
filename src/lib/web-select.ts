import { isDesktop } from "@/lib/transport";

/** Native `<select>` styling override for web mode.
 *
 * macOS WebKit and Linux Chromium render native select chrome differently
 * from each other and from Tauri's webview. Apply this `style` plus a
 * matching className to keep `<select>` elements consistent across web /
 * desktop. In Tauri (desktop), returns `undefined` so the native chrome
 * is preserved.
 */
export const webSelectStyle: React.CSSProperties | undefined = !isDesktop()
  ? {
      appearance: "none",
      backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='12' viewBox='0 0 8 12'%3E%3Cpath d='M4 1L7 4.5H1Z' fill='%23888'/%3E%3Cpath d='M4 11L1 7.5H7Z' fill='%23888'/%3E%3C/svg%3E")`,
      backgroundRepeat: "no-repeat",
      backgroundPosition: "right 8px center",
      paddingRight: "24px",
    }
  : undefined;

/** True in web mode — used to swap `rounded-[6px] h-[26px]` for the more
 *  permissive Tauri styling (`rounded-lg py-1.5`). */
export const isWeb = !isDesktop();
