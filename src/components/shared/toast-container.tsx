import { useToastStore } from "@/stores/toast-store";
import { clsx } from "clsx";
import { Check, X, Info } from "lucide-react";

const icons = {
  success: Check,
  error: X,
  info: Info,
};

export function ToastContainer() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 pointer-events-none">
      {toasts.map((t) => {
        const Icon = icons[t.type];
        return (
          <div
            key={t.id}
            className={clsx(
              "pointer-events-auto flex items-center gap-2 rounded-lg border px-3 py-2 text-sm shadow-lg animate-toast-in select-none",
              t.type === "success" && "border-primary bg-primary text-primary-foreground",
              t.type === "error" && "border-destructive bg-destructive text-destructive-foreground",
              t.type === "info" && "border-foreground bg-foreground text-background",
            )}
          >
            <Icon size={14} strokeWidth={2.5} className="shrink-0" />
            <span>{t.message}</span>
            <button
              onClick={() => dismiss(t.id)}
              className="ml-1 shrink-0 rounded p-0.5 opacity-60 hover:opacity-100 transition-opacity"
            >
              <X size={12} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
