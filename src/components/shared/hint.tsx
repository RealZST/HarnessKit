import { Lightbulb, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

interface HintProps {
  id: string;
  children: React.ReactNode;
  className?: string;
}

export function Hint({ id, children, className }: HintProps) {
  const { t } = useTranslation("common");
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
      className={`animate-fade-in flex items-center gap-3 rounded-lg border border-primary/20 bg-primary/10 px-4 py-3 shadow-sm ${className ?? ""}`}
    >
      <Lightbulb
        size={15}
        strokeWidth={1.75}
        className="shrink-0 text-primary"
      />
      <div className="min-w-0 flex-1 text-sm text-muted-foreground">
        {children}
      </div>
      <button
        onClick={dismiss}
        aria-label={t("dismissHint")}
        className="shrink-0 rounded p-2 text-muted-foreground/60 transition-colors hover:text-foreground"
      >
        <X size={14} />
      </button>
    </div>
  );
}
