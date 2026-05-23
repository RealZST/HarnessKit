import { useMemo } from "react";
import type { KitSummary } from "@/types/kits";

interface Props {
  kits: KitSummary[];
  selectedKitIds: string[];
  setSelectedKitIds: (ids: string[]) => void;
}

export function KitStep({ kits, selectedKitIds, setSelectedKitIds }: Props) {
  const selectedSet = useMemo(() => new Set(selectedKitIds), [selectedKitIds]);
  return (
    <div className="space-y-2">
      <ul className="max-h-64 space-y-1 overflow-auto rounded-md border">
        {kits.map((k) => (
          <li key={k.id}>
            <label className="flex cursor-pointer items-center gap-2 px-2 py-1 text-sm hover:bg-muted">
              <input
                type="checkbox"
                checked={selectedSet.has(k.id)}
                onChange={(e) => {
                  if (e.target.checked)
                    setSelectedKitIds([...selectedKitIds, k.id]);
                  else
                    setSelectedKitIds(
                      selectedKitIds.filter((id) => id !== k.id),
                    );
                }}
              />
              <span className="font-medium">{k.name}</span>
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
}
