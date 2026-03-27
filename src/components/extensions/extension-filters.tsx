import type { ExtensionKind } from "@/lib/types";
import { useExtensionStore } from "@/stores/extension-store";
import { Search, X } from "lucide-react";
import { clsx } from "clsx";

const TAG_COLORS = [
  "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
  "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400",
  "bg-teal-100 text-teal-700 dark:bg-teal-900/30 dark:text-teal-400",
  "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400",
  "bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-400",
  "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400",
  "bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-400",
  "bg-cyan-100 text-cyan-700 dark:bg-cyan-900/30 dark:text-cyan-400",
];

export function tagColor(index: number): string {
  return TAG_COLORS[index % TAG_COLORS.length];
}

export const CATEGORIES = [
  "Coding", "Testing", "DevOps", "Data", "Design",
  "Writing", "Education", "Finance", "Security",
  "Productivity", "Research", "Other",
] as const;

const kinds: (ExtensionKind | null)[] = [null, "skill", "mcp", "plugin", "hook"];

export function ExtensionFilters() {
  const { kindFilter, setKindFilter, searchQuery, setSearchQuery, allTags, tagFilter, setTagFilter, categoryFilter, setCategoryFilter } = useExtensionStore();

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-4">
        <div className="relative flex-1 max-w-sm">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search extensions..."
            className="w-full rounded-lg border border-border bg-card py-1.5 pl-9 pr-3 text-sm placeholder:text-muted-foreground focus:border-ring focus:outline-none"
          />
        </div>
        <select
          value={categoryFilter ?? ""}
          onChange={(e) => setCategoryFilter(e.target.value || null)}
          className="rounded-lg border border-border bg-card px-3 py-1.5 text-xs text-foreground focus:border-ring focus:outline-none"
        >
          <option value="">All Categories</option>
          {CATEGORIES.map((cat) => (
            <option key={cat} value={cat}>{cat}</option>
          ))}
        </select>
        <div className="flex gap-1.5">
          {kinds.map((kind) => (
            <button
              key={kind ?? "all"}
              onClick={() => setKindFilter(kind)}
              className={clsx(
                "rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
                kindFilter === kind
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground"
              )}
            >
              {kind ?? "All"}
            </button>
          ))}
        </div>
      </div>
      {allTags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {allTags.map((tag, i) => (
            <button
              key={tag}
              onClick={() => setTagFilter(tagFilter === tag ? null : tag)}
              className={clsx(
                "rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors",
                tagFilter === tag
                  ? tagColor(i) + " ring-2 ring-offset-1 ring-ring"
                  : tagColor(i) + " opacity-70 hover:opacity-100"
              )}
            >
              {tag}
              {tagFilter === tag && <X size={10} className="ml-1 inline" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
