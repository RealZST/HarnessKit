import { clsx } from "clsx";
import { Check, Search, X } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { KindBadge } from "@/components/shared/kind-badge";
import type { Extension, ExtensionKind } from "@/lib/types";
import { isWeb as web, webSelectStyle } from "@/lib/web-select";

interface Props {
  kindFilter: "skill" | "mcp" | "cli";
  selectedIds: string[];
  onSelectionChange(ids: string[]): void;
  candidates: Extension[];
}

export function EditorAssetTab({
  kindFilter,
  selectedIds,
  onSelectionChange,
  candidates,
}: Props) {
  const { t } = useTranslation("kits");
  const [search, setSearch] = useState("");
  const [pack, setPack] = useState<string | null>(null);

  const kindCandidates = useMemo(
    () => candidates.filter((c) => c.kind === (kindFilter as ExtensionKind)),
    [candidates, kindFilter],
  );

  const packs = useMemo(() => {
    const set = new Set<string>();
    for (const c of kindCandidates) if (c.pack) set.add(c.pack);
    return Array.from(set).sort();
  }, [kindCandidates]);

  const selectedSet = useMemo(() => new Set(selectedIds), [selectedIds]);

  const visible = useMemo(() => {
    const lo = search.toLowerCase();
    return kindCandidates.filter((c) => {
      if (pack && c.pack !== pack) return false;
      if (!lo) return true;
      return (
        c.name.toLowerCase().includes(lo) ||
        c.description.toLowerCase().includes(lo) ||
        c.kind.toLowerCase().includes(lo)
      );
    });
  }, [kindCandidates, search, pack]);

  function toggle(id: string) {
    if (selectedSet.has(id)) {
      onSelectionChange(selectedIds.filter((i) => i !== id));
    } else {
      onSelectionChange([...selectedIds, id]);
    }
  }

  // "Select all" operates over the currently-visible set (after search and
  // pack filter). Toggles to "Deselect all" once every visible row is in.
  const allVisibleSelected =
    visible.length > 0 && visible.every((v) => selectedSet.has(v.id));
  function toggleSelectAll() {
    const visibleIds = visible.map((v) => v.id);
    if (allVisibleSelected) {
      const visibleSet = new Set(visibleIds);
      onSelectionChange(selectedIds.filter((id) => !visibleSet.has(id)));
    } else {
      onSelectionChange(Array.from(new Set([...selectedIds, ...visibleIds])));
    }
  }

  return (
    <div className="flex h-full flex-col gap-3">
      {/* Filter row */}
      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search
            size={14}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
          />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("editor.searchPlaceholder")}
            aria-label={t("editor.searchPlaceholder")}
            className="w-full rounded-lg border border-border bg-card py-1.5 pl-8 pr-8 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none"
          />
          {search && (
            <button
              type="button"
              onClick={() => setSearch("")}
              aria-label={t("editor.clearSearch", "Clear search")}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            >
              <X size={14} />
            </button>
          )}
        </div>
        {packs.length > 0 && (
          <select
            value={pack ?? ""}
            onChange={(e) => setPack(e.target.value || null)}
            aria-label={t("editor.packFilterAll")}
            style={webSelectStyle}
            className={clsx(
              "w-36 shrink-0 overflow-hidden text-ellipsis border border-border bg-card px-3 text-xs text-foreground focus:border-ring focus:outline-none",
              web ? "rounded-[6px] h-[26px]" : "rounded-lg py-1.5",
            )}
          >
            <option value="">{t("editor.packFilterAll")}</option>
            {packs.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        )}
        {visible.length > 0 && (
          <button
            type="button"
            onClick={toggleSelectAll}
            className="whitespace-nowrap rounded-md border border-border bg-card px-2 py-1.5 text-xs text-foreground hover:bg-accent"
          >
            {allVisibleSelected
              ? t("editor.deselectAll")
              : t("editor.selectAll", { count: visible.length })}
          </button>
        )}
      </div>

      {/* List */}
      <ul className="flex-1 space-y-1 overflow-auto">
        {visible.length === 0 && (
          <li className="px-2 py-6 text-center text-sm text-muted-foreground">
            {t("editor.noMatches")}
          </li>
        )}
        {visible.map((ext) => {
          const isAdded = selectedSet.has(ext.id);
          return (
            <li key={ext.id}>
              {/* biome-ignore lint/a11y/useSemanticElements: role="checkbox" on <button> lets the whole row act as one toggle (label + badge + meta clickable together). */}
              <button
                type="button"
                role="checkbox"
                aria-checked={isAdded}
                onClick={() => toggle(ext.id)}
                className={`flex w-full items-center gap-3 rounded-md px-2 py-2 text-left hover:bg-muted ${
                  isAdded ? "bg-primary/5" : ""
                }`}
              >
                <span
                  aria-hidden
                  className={`flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors ${
                    isAdded
                      ? "border-primary bg-primary text-primary-foreground"
                      : "border-muted-foreground/40"
                  }`}
                >
                  {isAdded && <Check className="h-3 w-3" strokeWidth={3} />}
                </span>
                <KindBadge kind={ext.kind} />
                <span className="flex flex-col items-start truncate">
                  <span className="truncate text-sm">{ext.name}</span>
                  {ext.description && (
                    <span className="truncate text-xs text-muted-foreground">
                      {ext.description}
                    </span>
                  )}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
