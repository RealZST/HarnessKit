import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type SortingState,
} from "@tanstack/react-table";
import { useState } from "react";
import type { Extension } from "@/lib/types";
import { formatRelativeTime } from "@/lib/types";
import { KindBadge } from "@/components/shared/kind-badge";
import { PermissionTags } from "@/components/shared/permission-tags";
import { TrustBadge } from "@/components/shared/trust-badge";
import { useExtensionStore } from "@/stores/extension-store";

const col = createColumnHelper<Extension>();

const columns = [
  col.display({
    id: "select",
    header: () => {
      const { selectedIds, selectAll, clearSelection, filtered } = useExtensionStore();
      const allSelected = filtered().length > 0 && selectedIds.size === filtered().length;
      return (
        <input
          type="checkbox"
          checked={allSelected}
          onChange={() => allSelected ? clearSelection() : selectAll()}
          aria-label="Select all extensions"
          className="rounded border-border accent-primary"
        />
      );
    },
    cell: (info) => {
      const ext = info.row.original;
      const { selectedIds, toggleSelected } = useExtensionStore();
      return (
        <input
          type="checkbox"
          checked={selectedIds.has(ext.id)}
          onChange={(e) => { e.stopPropagation(); toggleSelected(ext.id); }}
          onClick={(e) => e.stopPropagation()}
          aria-label={`Select ${ext.name}`}
          className="rounded border-border accent-primary"
        />
      );
    },
    size: 40,
  }),
  col.accessor("name", {
    header: "Name",
    cell: (info) => {
      const ext = info.row.original;
      const status = useExtensionStore().updateStatuses.get(ext.id);
      const hasUpdate = status?.status === "update_available";
      return (
        <span className="font-medium">
          {info.getValue()}
          {hasUpdate && <span className="ml-1.5 inline-block h-2 w-2 rounded-full bg-primary" title="Update available" />}
        </span>
      );
    },
  }),
  col.accessor("kind", {
    header: "Kind",
    cell: (info) => <KindBadge kind={info.getValue()} />,
  }),
  col.accessor("agents", {
    header: "Agent",
    cell: (info) => <span className="text-muted-foreground">{info.getValue().join(", ")}</span>,
  }),
  col.accessor("permissions", {
    header: "Permissions",
    cell: (info) => <PermissionTags permissions={info.getValue()} />,
    enableSorting: false,
  }),
  col.accessor("trust_score", {
    header: "Score",
    cell: (info) => {
      const val = info.getValue();
      return val != null ? <TrustBadge score={val} size="sm" /> : <span className="text-muted-foreground">--</span>;
    },
  }),
  col.accessor("last_used_at", {
    header: "Last Used",
    cell: (info) => {
      const ext = info.row.original;
      if (ext.kind !== "skill") {
        return <span className="text-muted-foreground">—</span>;
      }
      const val = info.getValue();
      if (!val) {
        return <span className="text-muted-foreground">Unused</span>;
      }
      return <span className="text-muted-foreground">{formatRelativeTime(val)}</span>;
    },
  }),
  col.accessor("enabled", {
    header: "Status",
    cell: (info) => {
      const ext = info.row.original;
      const toggle = useExtensionStore().toggle;
      return (
        <button
          onClick={(e) => { e.stopPropagation(); toggle(ext.id, !ext.enabled); }}
          className={ext.enabled ? "text-primary text-xs" : "text-muted-foreground text-xs"}
        >
          {ext.enabled ? "enabled" : "disabled"}
        </button>
      );
    },
  }),
];

export function ExtensionTable({ data }: { data: Extension[] }) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const { selectedId, setSelectedId, searchQuery, kindFilter, tagFilter, categoryFilter } = useExtensionStore();
  const hasFilters = !!(searchQuery || kindFilter || tagFilter || categoryFilter);
  const table = useReactTable({
    data,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  return (
    <div className="rounded-xl border border-border overflow-hidden shadow-sm">
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead className="bg-muted/20">
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id}>
                {hg.headers.map((header) => (
                  <th
                    key={header.id}
                    scope="col"
                    className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-muted-foreground cursor-pointer select-none"
                    onClick={header.column.getToggleSortingHandler()}
                    style={header.column.getSize() ? { width: header.column.getSize() } : undefined}
                  >
                    {flexRender(header.column.columnDef.header, header.getContext())}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody className="divide-y divide-border">
            {table.getRowModel().rows.map((row) => (
              <tr
                key={row.id}
                onClick={() => setSelectedId(row.original.id === selectedId ? null : row.original.id)}
                className={`cursor-pointer transition-colors duration-150 ${
                  row.original.id === selectedId
                    ? "bg-accent"
                    : "hover:bg-muted/30"
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
        <div className="py-12 px-6 text-center">
          <h4 className="text-sm font-medium text-foreground">No extensions found</h4>
          <p className="mt-1 text-xs text-muted-foreground">
            {hasFilters
              ? "Try adjusting your filters."
              : "Browse the Marketplace to discover and install skills, MCP servers, and more."}
          </p>
        </div>
      )}
    </div>
  );
}
