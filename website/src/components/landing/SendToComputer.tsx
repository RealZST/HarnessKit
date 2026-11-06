import { Share } from "lucide-react";
import { useState } from "react";

interface T { label: string; meta: string; copied: string }

// Phones and tablets can't run the App: their first action sends this page to a computer. The share sheet where
// there is one (AirDrop, Messages, mail), else the link goes to the clipboard. Shown only on <html
// data-platform="mobile"> (GetButtons, DownloadPage). `statusOnly`: where a line nearby already says which computers
// it runs on (the hero's), the line under the button speaks only once something happens.
export default function SendToComputer({ t, statusOnly = false }: { t: T; statusOnly?: boolean }) {
  const [status, setStatus] = useState("");

  const send = async () => {
    const url = location.origin + location.pathname;
    if (navigator.share) {
      try {
        await navigator.share({ title: document.title, url });
        return;
      } catch (err) {
        // Closing the share sheet is an answer; anything else (no permission, nothing to share to) falls through.
        if (err instanceof DOMException && err.name === "AbortError") return;
      }
    }
    try {
      await navigator.clipboard.writeText(url);
      setStatus(t.copied);
    } catch {
      // No share sheet and no clipboard: the link itself, to copy by hand.
      setStatus(url);
    }
  };

  return (
    <div className="send">
      <button type="button" className="way" data-way="send" onClick={send}><Share size={16} aria-hidden="true" />{t.label}</button>
      {/* The facts until something happens (unless statusOnly), then what happened. */}
      <p className="meta" role="status">{status || (statusOnly ? "" : t.meta)}</p>
    </div>
  );
}
