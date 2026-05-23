import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { clsx } from "clsx";
import { Search, X } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { KindBadge } from "@/components/shared/kind-badge";
import type { Extension, ExtensionKind, GroupedExtension } from "@/lib/types";
import { isWeb as web, webSelectStyle } from "@/lib/web-select";
import { getCachedGroups } from "@/stores/extension-helpers";

const col = createColumnHelper<GroupedExtension>();

// Meta object threaded through TanStack Table — cell renderers pull selection
// state and the toggle callback off `info.table.options.meta` so the column
// array can stay reference-stable (no rebuild on every selection change),
// matching the pattern Extensions' table uses with store.getState().
interface TableMeta {
  selectedSet: Set<string>;
  toggle(group: GroupedExtension): void;
  selectAriaLabel(name: string): string;
}

interface Props {
  kindFilter: "skill" | "mcp" | "cli";
  selectedIds: string[];
  onSelectionChange(ids: string[]): void;
  candidates: Extension[];
}

/** Pick a primary instance from a group whose id should land in
 *  `extension_ids` when the group is selected. Latest-updated wins —
 *  same heuristic the (now-removed) backend dedup used. */
function primaryInstance(group: GroupedExtension): Extension {
  let winner = group.instances[0];
  for (const inst of group.instances) {
    if (inst.updated_at > winner.updated_at) winner = inst;
  }
  return winner;
}

// Columns declared at module scope so React never sees a fresh array — even
// the `useMemo([], [])` form was creating a new array on every mount, which
// re-keyed TanStack's internal column cache and forced a fresh getRowModel
// build per dialog open.
const COLUMNS = [
  col.display({
    id: "select",
    cell: (info) => {
      const meta = info.table.options.meta as TableMeta;
      const isAdded = info.row.original.instances.some((i) =>
        meta.selectedSet.has(i.id),
      );
      return (
        <input
          type="checkbox"
          checked={isAdded}
          onChange={() => meta.toggle(info.row.original)}
          onClick={(e) => e.stopPropagation()}
          aria-label={meta.selectAriaLabel(info.row.original.name)}
          className="rounded border-border accent-primary"
        />
      );
    },
    size: 32,
  }),
  col.display({
    id: "kind",
    cell: (info) => <KindBadge kind={info.row.original.kind} />,
    size: 64,
  }),
  col.display({
    id: "name",
    cell: (info) => {
      const g = info.row.original;
      return (
        <div className="flex flex-col">
          <span className="truncate text-sm">{g.name}</span>
          {g.description && (
            <span className="truncate text-xs text-muted-foreground">
              {g.description}
            </span>
          )}
        </div>
      );
    },
  }),
];

export function EditorAssetTab({
  kindFilter,
  selectedIds,
  onSelectionChange,
  candidates,
}: Props) {
  const { t } = useTranslation("kits");
  const [search, setSearch] = useState("");
  const [pack, setPack] = useState<string | null>(null);

  // Dedupe candidates with the exact same logic the Extensions page uses
  // so the Kit editor list never diverges from it. Use the cached groups
  // — the store pre-warms this cache during fetchCandidates, so opening
  // the dialog after the page-level idle prefetch is a cache hit and
  // pays zero buildGroups cost on the open paint.
  const kindGroups = useMemo(
    () =>
      getCachedGroups(candidates).filter(
        (g) => g.kind === (kindFilter as ExtensionKind),
      ),
    [candidates, kindFilter],
  );

  const packs = useMemo(() => {
    const set = new Set<string>();
    for (const g of kindGroups) if (g.pack) set.add(g.pack);
    return Array.from(set).sort();
  }, [kindGroups]);

  const selectedSet = useMemo(() => new Set(selectedIds), [selectedIds]);

  const visible = useMemo(() => {
    const lo = search.toLowerCase();
    return kindGroups.filter((g) => {
      if (pack && g.pack !== pack) return false;
      if (!lo) return true;
      return (
        g.name.toLowerCase().includes(lo) ||
        g.description.toLowerCase().includes(lo) ||
        g.kind.toLowerCase().includes(lo)
      );
    });
  }, [kindGroups, search, pack]);

  // A group counts as selected when ANY of its instance ids is in selection.
  // Toggle ON adds the primary instance id; toggle OFF removes every instance
  // id of that group (defensive: prior selections may carry sibling ids).
  function groupIsSelected(group: GroupedExtension): boolean {
    return group.instances.some((i) => selectedSet.has(i.id));
  }
  function toggle(group: GroupedExtension) {
    if (groupIsSelected(group)) {
      const groupIds = new Set(group.instances.map((i) => i.id));
      onSelectionChange(selectedIds.filter((id) => !groupIds.has(id)));
    } else {
      onSelectionChange([...selectedIds, primaryInstance(group).id]);
    }
  }

  // "Select all" operates over the currently-visible set (after search and
  // pack filter). Toggles to "Deselect all" once every visible row is in.
  const allVisibleSelected =
    visible.length > 0 && visible.every((g) => groupIsSelected(g));
  function toggleSelectAll() {
    if (allVisibleSelected) {
      const visibleIds = new Set(
        visible.flatMap((g) => g.instances.map((i) => i.id)),
      );
      onSelectionChange(selectedIds.filter((id) => !visibleIds.has(id)));
    } else {
      const additions = visible
        .filter((g) => !groupIsSelected(g))
        .map((g) => primaryInstance(g).id);
      onSelectionChange(Array.from(new Set([...selectedIds, ...additions])));
    }
  }

  // Meta is recomputed each render, but the columns array stays stable —
  // cell renderers pull from `info.table.options.meta`. This mirrors the
  // Extensions table's pattern of accessing live state from inside a stable
  // column definition.
  const tableMeta: TableMeta = useMemo(
    () => ({
      selectedSet,
      toggle: (group) => {
        if (group.instances.some((i) => selectedSet.has(i.id))) {
          const groupIds = new Set(group.instances.map((i) => i.id));
          onSelectionChange(selectedIds.filter((id) => !groupIds.has(id)));
        } else {
          onSelectionChange([...selectedIds, primaryInstance(group).id]);
        }
      },
      selectAriaLabel: (name) =>
        t("editor.selectItem", { defaultValue: "Select {{name}}", name }),
    }),
    [selectedSet, selectedIds, onSelectionChange, t],
  );

  const table = useReactTable({
    data: visible,
    columns: COLUMNS,
    meta: tableMeta,
    getCoreRowModel: getCoreRowModel(),
    getRowId: (row) => row.groupKey,
  });
  const rows = table.getRowModel().rows;

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

      {/* Table — mirrors Extensions list: stable column array, TanStack row
          model, plain <table> markup (browser table layout is cheaper than
          flex over many rows). No <thead> since rows aren't sortable here. */}
      <div className="flex-1 overflow-auto">
        <table className="w-full">
          <tbody>
            {rows.length === 0 && (
              <tr>
                <td
                  colSpan={COLUMNS.length}
                  className="px-2 py-6 text-center text-sm text-muted-foreground"
                >
                  {t("editor.noMatches")}
                </td>
              </tr>
            )}
            {rows.map((row) => {
              const isAdded = row.original.instances.some((i) =>
                selectedSet.has(i.id),
              );
              return (
                <tr
                  key={row.id}
                  onClick={() => toggle(row.original)}
                  className={clsx(
                    "cursor-pointer transition-colors hover:bg-muted",
                    isAdded && "bg-primary/5",
                  )}
                >
                  {row.getVisibleCells().map((cell) => (
                    // `select` and `kind` lock to their content width via
                    // `w-px whitespace-nowrap` (classic <table> trick) so the
                    // `name` column absorbs all remaining space. Without
                    // this, browsers redistribute leftover width when row
                    // descriptions are short (e.g. the MCP tab), leaving
                    // visible gaps around the checkbox and the KindBadge.
                    <td
                      key={cell.id}
                      className={clsx(
                        "px-2 py-2 align-middle",
                        (cell.column.id === "select" ||
                          cell.column.id === "kind") &&
                          "w-px whitespace-nowrap",
                      )}
                    >
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </td>
                  ))}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
