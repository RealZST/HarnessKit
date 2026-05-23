import { clsx } from "clsx";
import { Check, Eye, Search, X } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import {
  type AgentConfigFile,
  agentDisplayName,
  type ConfigCategory,
} from "@/lib/types";
import { isWeb as web, webSelectStyle } from "@/lib/web-select";
import type { KitConfigFileRef } from "@/types/kits";
import { FilePreviewModal } from "./file-preview-modal";

interface Props {
  selected: KitConfigFileRef[];
  onSelectionChange(refs: KitConfigFileRef[]): void;
  candidates: AgentConfigFile[];
}

// A config-file ref is identified by (agent, category, source_path) — the
// source_path lets a Kit hold multiple files for the same (agent, category)
// pair (e.g. two CLAUDE.md from different scopes). Install-side merges
// same-target entries by concatenating their contents.
const sameRef = (
  a: { agent: string; category: ConfigCategory; source_path: string | null },
  b: {
    agent: string;
    category: ConfigCategory;
    path?: string;
    source_path?: string | null;
  },
) => {
  const aPath = a.source_path ?? "";
  const bPath = b.source_path ?? b.path ?? "";
  return a.agent === b.agent && a.category === b.category && aPath === bPath;
};

type CategoryFilter = "all" | "rules" | "memory";

// Theme-aware tints for the per-row category badge. Both adapt to light/dark
// via the design tokens — rules uses a deeper foreground-tinted background,
// memory uses the lighter muted token.
const CATEGORY_BADGE_COLORS: Record<string, string> = {
  rules: "bg-foreground/15 text-foreground",
  memory: "bg-muted text-muted-foreground",
};

export function EditorConfigsTab({
  selected,
  onSelectionChange,
  candidates,
}: Props) {
  const { t } = useTranslation("kits");
  const [search, setSearch] = useState("");
  const [categoryFilter, setCategoryFilter] = useState<CategoryFilter>("all");
  const [agentFilter, setAgentFilter] = useState<string | null>(null);
  const [previewPath, setPreviewPath] = useState<string | null>(null);

  // Unique agents present in candidates, sorted for stable dropdown order.
  const agentOptions = useMemo(() => {
    const set = new Set<string>();
    for (const c of candidates) set.add(c.agent);
    return [...set].sort();
  }, [candidates]);

  const visible = useMemo(() => {
    const lo = search.toLowerCase();
    return candidates.filter((c) => {
      if (categoryFilter !== "all" && c.category !== categoryFilter)
        return false;
      if (agentFilter && c.agent !== agentFilter) return false;
      if (!lo) return true;
      return (
        c.agent.toLowerCase().includes(lo) ||
        c.category.toLowerCase().includes(lo) ||
        c.file_name.toLowerCase().includes(lo)
      );
    });
  }, [candidates, search, categoryFilter, agentFilter]);

  const isSelected = (c: AgentConfigFile) =>
    selected.some((s) =>
      sameRef(s, { agent: c.agent, category: c.category, path: c.path }),
    );

  // "Select all" covers the currently-visible set (after search + category
  // filter). Toggles to "Deselect all" once every visible row is in the set.
  const allVisibleSelected =
    visible.length > 0 && visible.every((c) => isSelected(c));
  function toggleSelectAll() {
    if (allVisibleSelected) {
      onSelectionChange(
        selected.filter(
          (s) =>
            !visible.some((c) =>
              sameRef(s, {
                agent: c.agent,
                category: c.category,
                path: c.path,
              }),
            ),
        ),
      );
    } else {
      const additions: KitConfigFileRef[] = [];
      for (const c of visible) {
        if (!isSelected(c)) {
          additions.push({
            agent: c.agent,
            category: c.category,
            source_path: c.path,
            source_file_name: c.file_name,
          });
        }
      }
      onSelectionChange([...selected, ...additions]);
    }
  }

  function toggle(c: AgentConfigFile) {
    if (isSelected(c)) {
      onSelectionChange(
        selected.filter(
          (s) =>
            !sameRef(s, { agent: c.agent, category: c.category, path: c.path }),
        ),
      );
    } else {
      onSelectionChange([
        ...selected,
        {
          agent: c.agent,
          category: c.category,
          source_path: c.path,
          source_file_name: c.file_name,
        },
      ]);
    }
  }

  return (
    <div className="flex h-full flex-col gap-3">
      {/* Search + Category filter + Select all */}
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
        {agentOptions.length > 0 && (
          <select
            value={agentFilter ?? ""}
            onChange={(e) => setAgentFilter(e.target.value || null)}
            aria-label={t("editor.agentFilter")}
            style={webSelectStyle}
            className={clsx(
              "w-32 shrink-0 overflow-hidden text-ellipsis border border-border bg-card px-3 text-xs text-foreground focus:border-ring focus:outline-none",
              web ? "rounded-[6px] h-[26px]" : "rounded-lg py-1.5",
            )}
          >
            <option value="">{t("editor.allAgents")}</option>
            {agentOptions.map((a) => (
              <option key={a} value={a}>
                {agentDisplayName(a)}
              </option>
            ))}
          </select>
        )}
        <select
          value={categoryFilter}
          onChange={(e) => setCategoryFilter(e.target.value as CategoryFilter)}
          aria-label={t("editor.configCategoryFilter")}
          style={webSelectStyle}
          className={clsx(
            "w-32 shrink-0 overflow-hidden text-ellipsis border border-border bg-card px-3 text-xs text-foreground focus:border-ring focus:outline-none",
            web ? "rounded-[6px] h-[26px]" : "rounded-lg py-1.5",
          )}
        >
          <option value="all">{t("editor.configCategoryAll")}</option>
          <option value="rules">{t("editor.configCategoryRules")}</option>
          <option value="memory">{t("editor.configCategoryMemory")}</option>
        </select>
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

      {/* Flat list */}
      <ul className="flex-1 space-y-1 overflow-auto">
        {visible.length === 0 && (
          <li className="px-2 py-6 text-center text-sm text-muted-foreground">
            {t("editor.noMatches")}
          </li>
        )}
        {visible.map((c) => {
          const sel = isSelected(c);
          return (
            <li key={`${c.agent}:${c.category}:${c.path}`}>
              {/* biome-ignore lint/a11y/useSemanticElements: role="checkbox" on <button> lets the whole row act as one toggle. */}
              <button
                type="button"
                role="checkbox"
                aria-checked={sel}
                onClick={() => toggle(c)}
                className={`flex w-full items-center gap-3 rounded-md px-2 py-2 text-left hover:bg-muted ${
                  sel ? "bg-primary/5" : ""
                }`}
              >
                <span
                  aria-hidden
                  className={`flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors ${
                    sel
                      ? "border-primary bg-primary text-primary-foreground"
                      : "border-muted-foreground/40"
                  }`}
                >
                  {sel && <Check className="h-3 w-3" strokeWidth={3} />}
                </span>
                <span
                  className={clsx(
                    "rounded px-1.5 py-0.5 text-xs uppercase tracking-wide",
                    CATEGORY_BADGE_COLORS[c.category] ??
                      "bg-muted text-muted-foreground",
                  )}
                >
                  {c.category}
                </span>
                <span
                  role="img"
                  title={agentDisplayName(c.agent)}
                  aria-label={agentDisplayName(c.agent)}
                  className="flex shrink-0 items-end justify-center"
                  style={{ width: 20, height: 20 }}
                >
                  <AgentMascot name={c.agent} size={18} />
                </span>
                <span className="flex-1 truncate text-sm">{c.file_name}</span>
                <span className="truncate text-xs text-muted-foreground">
                  {c.path}
                </span>
                {/* biome-ignore lint/a11y/useSemanticElements: cannot nest a real <button> inside the parent row button — span with role=button is the standard workaround. stopPropagation prevents the parent's checkbox toggle. */}
                <span
                  role="button"
                  tabIndex={0}
                  onClick={(e) => {
                    e.stopPropagation();
                    setPreviewPath(c.path);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.stopPropagation();
                      e.preventDefault();
                      setPreviewPath(c.path);
                    }
                  }}
                  aria-label={t("editor.previewFile", { name: c.file_name })}
                  title={t("editor.previewFile", { name: c.file_name })}
                  className="shrink-0 rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                >
                  <Eye className="h-3.5 w-3.5" />
                </span>
              </button>
            </li>
          );
        })}
      </ul>

      {previewPath && (
        <FilePreviewModal
          path={previewPath}
          onClose={() => setPreviewPath(null)}
        />
      )}
    </div>
  );
}
