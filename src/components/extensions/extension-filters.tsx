import type { ExtensionKind } from "@/lib/types";
import { useExtensionStore } from "@/stores/extension-store";
import { Search, X } from "lucide-react";
import { clsx } from "clsx";

const TAG_COLORS = [
  "bg-primary/10 text-primary",
  "bg-chart-1/10 text-chart-1",
  "bg-chart-2/10 text-chart-2",
  "bg-chart-3/10 text-chart-3",
  "bg-chart-4/10 text-chart-4",
  "bg-chart-5/10 text-chart-5",
  "bg-secondary/20 text-secondary-foreground",
  "bg-accent text-accent-foreground",
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
        <div className="relative flex-1 max-w-md">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search extensions..."
            aria-label="Search extensions"
            className="w-full rounded-lg border border-border bg-card py-1.5 pl-9 pr-3 text-sm placeholder:text-muted-foreground focus:border-ring focus:outline-none"
          />
        </div>
        <select
          value={categoryFilter ?? ""}
          onChange={(e) => setCategoryFilter(e.target.value || null)}
          aria-label="Filter by category"
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
              aria-pressed={kindFilter === kind}
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
              aria-pressed={tagFilter === tag}
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
