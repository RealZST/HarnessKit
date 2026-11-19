import { Loader2, X } from "lucide-react";
import { type ReactNode, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

/** The panel that slides in over the right of a Marketplace list for the item
 *  picked in it. It sits in the nearest positioned ancestor, and on a narrow
 *  window, where that leaves it no room, covers the window instead.
 *  `itemKey` names the item: a new one takes focus and starts at the top. */
export function DetailPanel({
  itemKey,
  heading,
  onClose,
  children,
}: {
  itemKey: string;
  heading: ReactNode;
  onClose: () => void;
  children: ReactNode;
}) {
  const { t } = useTranslation("marketplace");
  const panel = useRef<HTMLDivElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `itemKey` is read to take focus again for each item.
  useEffect(() => {
    panel.current?.focus({ preventScroll: true });
  }, [itemKey]);

  return (
    <div className="fixed inset-3 z-50 sm:absolute sm:inset-y-0 sm:left-auto sm:right-0 sm:z-10 sm:w-96">
      <div
        ref={panel}
        tabIndex={-1}
        onWheel={(e) => e.stopPropagation()}
        className="animate-slide-in-right flex h-full flex-col rounded-xl border border-border bg-card shadow-sm outline-none"
      >
        <div className="flex shrink-0 items-start justify-between border-b border-border px-5 py-4">
          <div className="min-w-0">{heading}</div>
          <button
            onClick={onClose}
            aria-label={t("detail.closeAria")}
            className="shrink-0 rounded-lg p-2.5 text-muted-foreground hover:text-foreground"
          >
            <X size={18} />
          </button>
        </div>
        <div
          key={itemKey}
          className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-5 py-4"
        >
          {children}
        </div>
      </div>
    </div>
  );
}

/** A skill's SKILL.md inside a `DetailPanel`: `undefined` while it loads,
 *  `null` when there is none to show. */
export function SkillDocumentation({
  content,
}: {
  content: string | null | undefined;
}) {
  const { t } = useTranslation("marketplace");
  return (
    <div className="mt-4">
      <h4 className="mb-2 border-b border-border pb-1 text-xs font-medium text-muted-foreground">
        {t("detail.documentation")}
      </h4>
      <div className="rounded-lg border border-border bg-card p-3">
        {content === undefined ? (
          <div className="flex justify-center py-8">
            <Loader2 size={20} className="animate-spin text-muted-foreground" />
          </div>
        ) : content ? (
          <pre className="whitespace-pre-wrap break-words text-xs text-muted-foreground">
            {content}
          </pre>
        ) : (
          <p className="text-xs italic text-muted-foreground">
            {t("detail.noPreview")}
          </p>
        )}
      </div>
    </div>
  );
}
