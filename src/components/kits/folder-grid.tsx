import { type ReactNode, useMemo } from "react";
import type { KitSummary } from "@/types/kits";
import { FolderCard } from "./folder-card";

interface Props {
  kits: KitSummary[];
  activeKitId: string | null;
  selectedKitIds: string[];
  onOpenDetail(id: string): void;
  onSelectionChange(ids: string[]): void;
  /** Optional extra grid cells rendered AFTER all kits (e.g. ghost tiles
   *  for "New Kit" / "Import Kit"). Hidden by the caller when not relevant. */
  trailingChildren?: ReactNode;
}

export function FolderGrid({
  kits,
  activeKitId,
  selectedKitIds,
  onOpenDetail,
  onSelectionChange,
  trailingChildren,
}: Props) {
  const selectedSet = useMemo(() => new Set(selectedKitIds), [selectedKitIds]);

  function toggle(id: string) {
    if (selectedSet.has(id)) {
      onSelectionChange(selectedKitIds.filter((x) => x !== id));
    } else {
      onSelectionChange([...selectedKitIds, id]);
    }
  }

  return (
    <div
      className="grid justify-start gap-x-6 gap-y-8"
      style={{ gridTemplateColumns: "repeat(auto-fill, minmax(146px, 166px))" }}
    >
      {kits.map((k) => (
        <FolderCard
          key={k.id}
          name={k.name}
          kindCounts={k.kind_counts}
          configCount={k.config_file_count}
          selected={selectedSet.has(k.id)}
          panelOpen={activeKitId === k.id}
          onOpenDetail={() => onOpenDetail(k.id)}
          onToggleSelect={() => toggle(k.id)}
        />
      ))}
      {trailingChildren}
    </div>
  );
}
