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
import { KindBadge } from "@/components/shared/kind-badge";
import { PermissionTags } from "@/components/shared/permission-tags";
import { TrustBadge } from "@/components/shared/trust-badge";
import { useExtensionStore } from "@/stores/extension-store";

const col = createColumnHelper<Extension>();

const columns = [
  col.display({
    id: "select",
    header: () => {
      const { selectedIds, selectAll, clearSelection, filtered } = useExtensionStore.getState();
      const allSelected = filtered().length > 0 && selectedIds.size === filtered().length;
      return (
        <input
          type="checkbox"
          checked={allSelected}
          onChange={() => allSelected ? clearSelection() : selectAll()}
          className="rounded"
        />
      );
    },
    cell: (info) => {
      const ext = info.row.original;
      const { selectedIds, toggleSelected } = useExtensionStore.getState();
      return (
        <input
          type="checkbox"
          checked={selectedIds.has(ext.id)}
          onChange={(e) => { e.stopPropagation(); toggleSelected(ext.id); }}
          onClick={(e) => e.stopPropagation()}
          className="rounded"
        />
      );
    },
    size: 40,
  }),
  col.accessor("name", {
    header: "Name",
    cell: (info) => {
      const ext = info.row.original;
      const status = useExtensionStore.getState().updateStatuses.get(ext.id);
      const hasUpdate = status?.status === "update_available";
      return (
        <span className="font-medium">
          {info.getValue()}
          {hasUpdate && <span className="ml-1.5 inline-block h-2 w-2 rounded-full bg-blue-500" title="Update available" />}
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
  col.accessor("enabled", {
    header: "Status",
    cell: (info) => {
      const ext = info.row.original;
      const toggle = useExtensionStore.getState().toggle;
      return (
        <button
          onClick={(e) => { e.stopPropagation(); toggle(ext.id, !ext.enabled); }}
          className={ext.enabled ? "text-green-500 dark:text-green-400 text-xs" : "text-red-500 dark:text-red-400 text-xs"}
        >
          {ext.enabled ? "enabled" : "disabled"}
        </button>
      );
    },
  }),
];

export function ExtensionTable({ data }: { data: Extension[] }) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const { selectedId, setSelectedId } = useExtensionStore();
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
      <table className="w-full">
        <thead className="bg-muted">
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id}>
              {hg.headers.map((header) => (
                <th
                  key={header.id}
                  className="px-4 py-3 text-left text-xs font-medium text-muted-foreground cursor-pointer select-none"
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
              className={`cursor-pointer transition-colors ${
                row.original.id === selectedId
                  ? "bg-accent"
                  : "hover:bg-muted"
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
      {data.length === 0 && (
        <div className="py-12 text-center text-muted-foreground">No extensions found</div>
      )}
    </div>
  );
}
