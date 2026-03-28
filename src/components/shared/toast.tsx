import { useEffect, useRef } from "react";
import { X } from "lucide-react";

interface ToastProps {
  message: string;
  onUndo: () => void;
  onDismiss: () => void;
  duration?: number;
}

export function Toast({ message, onUndo, onDismiss, duration = 5000 }: ToastProps) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    timerRef.current = setTimeout(onDismiss, duration);
    return () => { if (timerRef.current) clearTimeout(timerRef.current); };
  }, [onDismiss, duration]);

  return (
    <div
      role="status"
      aria-live="polite"
      className="animate-toast-in fixed bottom-6 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-lg bg-foreground px-4 py-3 shadow-lg"
    >
      <span className="text-sm text-background">{message}</span>
      <button
        onClick={onUndo}
        className="min-h-[44px] min-w-[44px] rounded-lg bg-primary px-3 py-1 text-sm font-medium text-primary-foreground transition-colors duration-200 hover:bg-primary/90"
      >
        Undo
      </button>
      <button
        onClick={onDismiss}
        aria-label="Dismiss notification"
        className="min-h-[44px] min-w-[44px] rounded p-2 text-background/60 transition-colors duration-200 hover:text-background"
      >
        <X size={14} />
      </button>
    </div>
  );
}
