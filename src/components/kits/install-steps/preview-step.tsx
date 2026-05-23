import type { TFunction } from "i18next";
import { useMemo } from "react";
import type { KitConflictPreview, KitSummary } from "@/types/kits";

type TFn = TFunction<"kits">;

export interface PreviewEntry {
  kit_id: string;
  agent_name: string;
  preview: KitConflictPreview;
}

interface Props {
  previews: PreviewEntry[];
  kits: KitSummary[];
  forceExtIds: Set<string>;
  setForceExtIds: (s: Set<string>) => void;
  forceCfgKeys: Set<string>;
  setForceCfgKeys: (s: Set<string>) => void;
  t: TFn;
}

export function PreviewStep({
  previews,
  kits,
  forceExtIds,
  setForceExtIds,
  forceCfgKeys,
  setForceCfgKeys,
  t,
}: Props) {
  const kitNameById = useMemo(() => {
    const map = new Map<string, string>();
    for (const k of kits) map.set(k.id, k.name);
    return map;
  }, [kits]);

  const allExtIds = useMemo(() => {
    const out = new Set<string>();
    for (const p of previews)
      for (const c of p.preview.extension_conflicts) out.add(c.extension_id);
    return out;
  }, [previews]);
  const allCfgKeys = useMemo(() => {
    const out = new Set<string>();
    for (const p of previews)
      for (const c of p.preview.config_conflicts)
        out.add(`${c.agent}:${c.category}`);
    return out;
  }, [previews]);
  const totalConflicts = allExtIds.size + allCfgKeys.size;
  const allOverwriteOn =
    totalConflicts > 0 &&
    [...allExtIds].every((id) => forceExtIds.has(id)) &&
    [...allCfgKeys].every((k) => forceCfgKeys.has(k));

  return (
    <div className="space-y-3">
      {totalConflicts > 1 && (
        // Right-aligned so the checkbox visually sits above the column of
        // per-row Overwrite toggles inside each conflict group; padding
        // matches each group card's `p-3`.
        <div className="flex justify-end px-3">
          <label className="flex cursor-pointer items-center gap-1 text-xs font-medium">
            <input
              type="checkbox"
              checked={allOverwriteOn}
              onChange={() => {
                if (allOverwriteOn) {
                  setForceExtIds(new Set());
                  setForceCfgKeys(new Set());
                } else {
                  setForceExtIds(new Set(allExtIds));
                  setForceCfgKeys(new Set(allCfgKeys));
                }
              }}
              className="accent-destructive"
            />
            <span>
              {t("previewDialog.overwriteAll", {
                defaultValue: "Overwrite all",
              })}
            </span>
          </label>
        </div>
      )}
      {previews.map((entry) => {
        const total =
          entry.preview.extension_conflicts.length +
          entry.preview.config_conflicts.length;
        if (total === 0) return null;
        const kitName = kitNameById.get(entry.kit_id) ?? entry.kit_id;
        return (
          <div
            key={`${entry.kit_id}:${entry.agent_name}`}
            className="rounded-md border border-border bg-muted/30 p-3"
          >
            <p className="mb-2 text-xs font-medium">
              {kitName}{" "}
              <span className="text-muted-foreground">
                → {entry.agent_name}
              </span>
            </p>
            <ul className="space-y-1 text-sm">
              {entry.preview.extension_conflicts.map((c) => (
                <li key={c.extension_id} className="flex items-center gap-2">
                  <span className="text-warning">⚠</span>
                  <span className="flex-1 truncate">{c.asset_name}</span>
                  <label className="flex items-center gap-1 text-xs">
                    <input
                      type="checkbox"
                      checked={forceExtIds.has(c.extension_id)}
                      onChange={(e) => {
                        const next = new Set(forceExtIds);
                        if (e.target.checked) next.add(c.extension_id);
                        else next.delete(c.extension_id);
                        setForceExtIds(next);
                      }}
                    />
                    {t("previewDialog.overwriteLabel")}
                  </label>
                </li>
              ))}
              {entry.preview.config_conflicts.map((c) => {
                const key = `${c.agent}:${c.category}`;
                return (
                  <li key={key} className="flex items-center gap-2">
                    <span className="text-warning">⚠</span>
                    <span className="flex-1 truncate">{c.target_path}</span>
                    <label className="flex items-center gap-1 text-xs">
                      <input
                        type="checkbox"
                        checked={forceCfgKeys.has(key)}
                        onChange={(e) => {
                          const next = new Set(forceCfgKeys);
                          if (e.target.checked) next.add(key);
                          else next.delete(key);
                          setForceCfgKeys(next);
                        }}
                      />
                      {t("previewDialog.overwriteLabel")}
                    </label>
                  </li>
                );
              })}
            </ul>
          </div>
        );
      })}
    </div>
  );
}
