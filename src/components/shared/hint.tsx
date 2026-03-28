import { useState } from "react";
import { X, Lightbulb } from "lucide-react";

interface HintProps {
  id: string;
  children: React.ReactNode;
  className?: string;
}

export function Hint({ id, children, className }: HintProps) {
  const storageKey = `hk-hint-${id}`;
  const [visible, setVisible] = useState(
    () => localStorage.getItem(storageKey) !== "dismissed",
  );

  if (!visible) return null;

  const dismiss = () => {
    localStorage.setItem(storageKey, "dismissed");
    setVisible(false);
  };

  return (
    <div
      className={`animate-fade-in flex items-start gap-3 rounded-lg border border-primary/15 bg-primary/8 px-4 py-3 ${className ?? ""}`}
    >
      <Lightbulb
        size={15}
        strokeWidth={1.75}
        className="mt-0.5 shrink-0 text-primary"
      />
      <div className="min-w-0 flex-1 text-sm text-muted-foreground">
        {children}
      </div>
      <button
        onClick={dismiss}
        aria-label="Dismiss hint"
        className="shrink-0 rounded p-2 text-muted-foreground/60 transition-colors hover:text-foreground"
      >
        <X size={14} />
      </button>
    </div>
  );
}
