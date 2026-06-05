import { useState } from "react";
import { useTranslation } from "react-i18next";

interface HermesCategoryPickerProps {
  /** Existing category names under `~/.hermes/skills/`. */
  categories: string[];
  /** Currently selected category, or the in-progress name when adding a new one. */
  value: string;
  /** Reports the effective category (a picked name, or the typed new name). */
  onChange: (category: string) => void;
  disabled?: boolean;
}

/**
 * Category selector for Hermes skill installs — a pill row of existing
 * categories plus a "+ New" affordance that swaps in a free-text input.
 *
 * Self-contained: the new-vs-pick mode lives here and the parent only tracks the
 * resulting category string via `onChange`. Callers should still coerce an empty
 * value to a sensible default (e.g. `value.trim() || "local"`) at submit time.
 */
export function HermesCategoryPicker({
  categories,
  value,
  onChange,
  disabled,
}: HermesCategoryPickerProps) {
  const { t } = useTranslation("common");
  const [newMode, setNewMode] = useState(false);

  if (newMode) {
    return (
      <div className="flex items-center gap-1.5">
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={t("hermesCategory.newPlaceholder")}
          className="flex-1 rounded-lg border border-border bg-background px-2.5 py-1 text-xs outline-none focus:border-ring focus:ring-2 focus:ring-ring/50"
          disabled={disabled}
          // biome-ignore lint/a11y/noAutofocus: new-category mode is opt-in (user clicked "+ New"); focusing the input they just summoned is the expected behavior, not a surprise focus trap.
          autoFocus
        />
        <button
          type="button"
          onClick={() => {
            setNewMode(false);
            onChange(categories[0] ?? "local");
          }}
          disabled={disabled}
          className="text-xs text-muted-foreground hover:text-foreground"
        >
          {t("cancel")}
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-wrap gap-1.5">
      {categories.map((cat) => (
        <button
          type="button"
          key={cat}
          onClick={() => onChange(cat)}
          disabled={disabled}
          className={`rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors ${
            value === cat
              ? "bg-primary/20 text-primary"
              : "bg-muted text-muted-foreground hover:bg-muted/80"
          }`}
        >
          {cat}
        </button>
      ))}
      <button
        type="button"
        onClick={() => {
          setNewMode(true);
          onChange("");
        }}
        disabled={disabled}
        className="rounded-full px-2.5 py-0.5 text-xs font-medium bg-muted text-muted-foreground hover:bg-muted/80 transition-colors"
      >
        {t("hermesCategory.addNew")}
      </button>
    </div>
  );
}
