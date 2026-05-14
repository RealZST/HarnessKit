import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  type SortingState,
  useReactTable,
} from "@tanstack/react-table";
import { ChevronDown, ChevronUp } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import { KindBadge } from "@/components/shared/kind-badge";
import { PermissionTags } from "@/components/shared/permission-tags";
import { TrustBadge } from "@/components/shared/trust-badge";
import { useScope } from "@/hooks/use-scope";
import type { ExtensionKind, GroupedExtension } from "@/lib/types";
import { agentDisplayName, sortAgentNames } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

const col = createColumnHelper<GroupedExtension>();

export function ExtensionTable({
  data,
  scrollToId,
}: {
  data: GroupedExtension[];
  scrollToId?: string | null;
}) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const { scope } = useScope();
  const navigate = useNavigate();
  // Subscribe to trigger re-render; accessed via getState() in cell renderers
  useExtensionStore((s) => s.selectedIds);
  const selectAll = useExtensionStore((s) => s.selectAll);
  const clearSelection = useExtensionStore((s) => s.clearSelection);
  const toggleSelected = useExtensionStore((s) => s.toggleSelected);
  // Subscribe to trigger re-render; accessed via getState() in cell renderers
  useExtensionStore((s) => s.updateStatuses);
  const toggle = useExtensionStore((s) => s.toggle);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `scope` is a trigger sentinel — cell renderers read scope-dependent filter results via getState(); listing it forces a column rebuild on scope change.
  const columns = useMemo(
    () => [
      col.display({
        id: "select",
        header: () => {
          const ids = useExtensionStore.getState().selectedIds;
          const all = useExtensionStore.getState().filtered();
          const allSelected = all.length > 0 && ids.size === all.length;
          return (
            <input
              type="checkbox"
              checked={allSelected}
              onChange={() => (allSelected ? clearSelection() : selectAll())}
              aria-label={t("table.selectAll")}
              className="rounded border-border accent-primary"
            />
          );
        },
        cell: (info) => {
          const ext = info.row.original;
          const ids = useExtensionStore.getState().selectedIds;
          return (
            <input
              type="checkbox"
              checked={ids.has(ext.groupKey)}
              onChange={(e) => {
                e.stopPropagation();
                toggleSelected(ext.groupKey);
              }}
              onClick={(e) => e.stopPropagation()}
              aria-label={t("table.selectItem", { name: ext.name })}
              className="rounded border-border accent-primary"
            />
          );
        },
        size: 40,
      }),
      col.accessor("name", {
        header: () => t("table.headers.name"),
        sortingFn: (a, b) =>
          a.original.name.localeCompare(b.original.name, undefined, {
            sensitivity: "base",
          }),
        cell: (info) => {
          const ext = info.row.original;
          const statuses = useExtensionStore.getState().updateStatuses;
          const hasUpdate = ext.instances.some(
            (inst) => statuses.get(inst.id)?.status === "update_available",
          );
          // Friendly name for hooks: "afplay Glass.aiff" (command with paths stripped)
          let displayName = info.getValue();
          if (ext.kind === "hook") {
            const parts = ext.name.split(":");
            if (parts.length >= 3) {
              const cmd = parts.slice(2).join(":");
              // Strip directory paths from each token: "/usr/bin/afplay /System/Library/Sounds/Glass.aiff" → "afplay Glass.aiff"
              displayName = cmd
                .split(" ")
                .map((t) => t.split("/").pop() || t)
                .join(" ");
            }
          }
          return (
            <span className="flex items-center gap-2 font-medium">
              {hasUpdate && (
                <span
                  className="inline-block h-2 w-2 shrink-0 rounded-full bg-primary"
                  title={t("table.updateAvailable")}
                />
              )}
              <span>{displayName}</span>
            </span>
          );
        },
      }),
      col.accessor("kind", {
        header: () => t("table.headers.kind"),
        cell: (info) => <KindBadge kind={info.getValue()} />,
      }),
      col.accessor("agents", {
        header: () => t("table.headers.agent"),
        cell: (info) => (
          <div className="flex items-end gap-1">
            {sortAgentNames(info.getValue(), agentOrder).map((name) => (
              <div
                key={name}
                title={agentDisplayName(name)}
                className="flex items-end justify-center"
                style={{ width: 20, height: 20 }}
              >
                <AgentMascot name={name} size={18} />
              </div>
            ))}
          </div>
        ),
      }),
      col.accessor("permissions", {
        header: () => t("table.headers.permissions"),
        cell: (info) => <PermissionTags permissions={info.getValue()} />,
        enableSorting: false,
      }),
      col.accessor("trust_score", {
        header: () => t("table.headers.audit"),
        cell: (info) => {
          const val = info.getValue();
          return val != null ? (
            <TrustBadge score={val} size="sm" />
          ) : (
            <span className="text-muted-foreground">--</span>
          );
        },
      }),
      col.accessor("enabled", {
        header: () => t("table.headers.status"),
        cell: (info) => {
          const ext = info.row.original;
          return (
            <button
              onClick={async (e) => {
                e.stopPropagation();
                const toastName =
                  ext.kind === "hook" && ext.name.includes(":")
                    ? ext.name
                        .split(":")
                        .slice(2)
                        .join(":")
                        .split(" ")
                        .map((t) => t.split("/").pop() || t)
                        .join(" ")
                    : ext.name;
                const action = ext.enabled
                  ? t("table.disabled")
                  : t("table.enabled");
                const ok = await toggle(ext.groupKey, !ext.enabled);
                if (ok) {
                  toast.success(
                    t("table.toggleSuccess", { name: toastName, action }),
                  );
                } else {
                  toast.error(
                    t("table.toggleFailed", { name: toastName, action }),
                  );
                }
              }}
              aria-label={t("table.toggle", { name: ext.name })}
              className={
                ext.enabled
                  ? "cursor-pointer rounded-full px-2.5 py-0.5 text-xs font-medium bg-primary/15 text-primary hover:bg-primary/20 transition-colors"
                  : "cursor-pointer rounded-full px-2.5 py-0.5 text-xs font-medium bg-muted text-muted-foreground hover:bg-muted/80 transition-colors"
              }
            >
              {ext.enabled ? t("table.enabled") : t("table.disabled")}
            </button>
          );
        },
      }),
    ],
    // selectedIds, updateStatuses accessed via getState() inside cell renderers
    // to avoid recomputing columns on every selection/status change.
    [agentOrder, selectAll, clearSelection, toggleSelected, toggle, scope, t],
  );
  const sorting = useExtensionStore((s) => s.tableSorting) as SortingState;
  const setStoreSorting = useExtensionStore((s) => s.setTableSorting);
  const setSorting = useCallback(
    (updater: SortingState | ((prev: SortingState) => SortingState)) => {
      const next =
        typeof updater === "function"
          ? updater(useExtensionStore.getState().tableSorting as SortingState)
          : updater;
      setStoreSorting(next);
    },
    [setStoreSorting],
  );
  const selectedId = useExtensionStore((s) => s.selectedId);
  const setSelectedId = useExtensionStore((s) => s.setSelectedId);
  const searchQuery = useExtensionStore((s) => s.searchQuery);
  const kindFilter = useExtensionStore((s) => s.kindFilter);
  const tagFilter = useExtensionStore((s) => s.tagFilter);
  const packFilter = useExtensionStore((s) => s.packFilter);
  const hasFilters = !!(searchQuery || kindFilter || tagFilter || packFilter);
  const tableContainerRef = useRef<HTMLDivElement>(null);
  const table = useReactTable({
    data,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const rows = table.getRowModel().rows;

  // Scroll to a specific row only when navigating from outside (e.g., overview page).
  // Does NOT scroll when user clicks rows in the list.
  const lastScrolledRef = useRef<string | null>(null);

  useEffect(() => {
    if (!scrollToId || scrollToId === lastScrolledRef.current) return;
    const row = rows.find((r) => r.original.groupKey === scrollToId);
    if (!row) return;
    lastScrolledRef.current = scrollToId;
    requestAnimationFrame(() => {
      const el = document.getElementById(`ext-row-${row.id}`);
      if (el) el.scrollIntoView({ block: "center", behavior: "instant" });
    });
  }, [scrollToId, rows]);

  return (
    <div
      ref={tableContainerRef}
      className="rounded-xl border border-border overflow-hidden shadow-sm"
    >
      <div className="overflow-x-auto">
        <table className="w-full" aria-label={t("table.ariaLabel")}>
          <thead className="bg-muted/30">
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id}>
                {hg.headers.map((header) => (
                  <th
                    key={header.id}
                    scope="col"
                    className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-muted-foreground cursor-pointer select-none"
                    onClick={header.column.getToggleSortingHandler()}
                    style={
                      header.column.getSize()
                        ? { width: header.column.getSize() }
                        : undefined
                    }
                  >
                    {flexRender(
                      header.column.columnDef.header,
                      header.getContext(),
                    )}
                    {header.column.getIsSorted() === "asc" && (
                      <ChevronUp
                        size={12}
                        className="ml-1 inline text-foreground"
                        aria-hidden="true"
                      />
                    )}
                    {header.column.getIsSorted() === "desc" && (
                      <ChevronDown
                        size={12}
                        className="ml-1 inline text-foreground"
                        aria-hidden="true"
                      />
                    )}
                    {!header.column.getIsSorted() &&
                      header.column.getCanSort() && (
                        <ChevronUp
                          size={12}
                          className="ml-1 inline text-muted-foreground/50"
                          aria-hidden="true"
                        />
                      )}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody className="divide-y divide-border">
            {rows.map((row) => (
              <tr
                key={row.id}
                id={`ext-row-${row.id}`}
                onClick={() =>
                  setSelectedId(
                    row.original.groupKey === selectedId
                      ? null
                      : row.original.groupKey,
                  )
                }
                className={`cursor-pointer transition-colors duration-150 ${
                  row.original.groupKey === selectedId
                    ? "bg-accent border-l-2 border-l-primary"
                    : "hover:bg-muted/40"
                }`}
              >
                {row.getVisibleCells().map((cell) => (
                  <td key={cell.id} className="px-4 py-3 text-sm">
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {data.length === 0 && (
        <div className="py-12 px-6 text-left">
          {scope.type === "project" ? (
            <>
              <h4 className="text-sm font-medium text-foreground">
                {t("table.emptyScope", { scope: scope.name })}
              </h4>
              <p className="mt-1 text-xs text-muted-foreground">
                {t("table.emptyScopeHint")}
              </p>
              <button
                onClick={() => navigate("/marketplace")}
                className="mt-3 rounded bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90"
              >
                {t("table.browseMarketplace")}
              </button>
            </>
          ) : hasFilters ? (
            <p className="text-sm text-muted-foreground">
              {t("table.noFilterMatch", {
                kind: kindFilter
                  ? tc(`kinds.${kindFilter as ExtensionKind}` as const)
                  : tc("kinds.all").toLowerCase(),
              })}
              <button
                onClick={() => {
                  useExtensionStore.getState().setSearchQuery("");
                  useExtensionStore.getState().setKindFilter(null);
                  useExtensionStore.getState().setTagFilter(null);
                  useExtensionStore.getState().setPackFilter(null);
                }}
                className="ml-1 font-medium text-foreground/70 hover:text-foreground transition-colors"
              >
                {t("table.clearFilters")}
              </button>
            </p>
          ) : (
            <>
              <h4 className="text-sm font-medium text-foreground">
                {t("table.noKindFound", {
                  kind: kindFilter
                    ? tc(`kinds.${kindFilter as ExtensionKind}` as const)
                    : tc("kinds.all").toLowerCase(),
                })}
              </h4>
              <p className="mt-1 text-xs text-muted-foreground">
                {t("table.discoverHint")}
              </p>
            </>
          )}
        </div>
      )}
    </div>
  );
}
