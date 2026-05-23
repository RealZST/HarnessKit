import { Check } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { KindCounts } from "@/types/kits";

interface Props {
  name: string;
  kindCounts: KindCounts;
  configCount: number;
  selected: boolean;
  panelOpen: boolean;
  onOpenDetail(): void;
  onToggleSelect(): void;
}

// Kits only surface file/skill/mcp here (no CLI / hook / plugin papers or pills),
// in this exact order. The `tint` class for each paper is the SAME bg class
// the pill uses — the colored sheet peeking out should read as the same hue
// as its pill so the visual binding is obvious. Tailwind's `/15` already
// resolves through `--kind-*` (theme-aware) and applies alpha against
// whatever sits behind, so we don't need a separate `--paper-*` token layer.
const PAPER_ORDER: Array<{
  key: keyof KindCounts | "config";
  tint: string;
  pill: string;
  label: string;
}> = [
  {
    key: "config",
    tint: "bg-paper-config",
    pill: "bg-muted text-muted-foreground ring-muted-foreground/20",
    label: "FILE",
  },
  {
    key: "skill",
    tint: "bg-paper-skill",
    pill: "bg-kind-skill/15 text-kind-skill ring-kind-skill/30",
    label: "SKILL",
  },
  {
    key: "mcp",
    tint: "bg-paper-mcp",
    pill: "bg-kind-mcp/15 text-kind-mcp ring-kind-mcp/30",
    label: "MCP",
  },
];

// Mirrors folders.html "Inspiration" layout: the FRONT-most paper sits in
// the center with a small upward translate; the two behind it fan out
// LEFT and RIGHT. Since z-index is computed as `visiblePapers.length - idx`,
// PAPER_ORDER[0] (FILE) is the front centered paper, [1] (SKILL) fans left,
// [2] (MCP) fans right. All three peek at the top of the folder body.
const TILTS = [
  { x: "0%", y: "-2%", rot: "0deg" },
  { x: "-14%", y: "-6%", rot: "-4deg" },
  { x: "14%", y: "-4%", rot: "3deg" },
];

export function FolderCard({
  name,
  kindCounts,
  configCount,
  selected,
  panelOpen,
  onOpenDetail,
  onToggleSelect,
}: Props) {
  const { t } = useTranslation("kits");
  const [showCheckbox, setShowCheckbox] = useState(false);

  const visiblePapers = PAPER_ORDER.filter(({ key }) => {
    if (key === "config") return configCount > 0;
    return kindCounts[key] > 0;
  });
  const pillEntries = visiblePapers.map((p) => ({
    ...p,
    count: p.key === "config" ? configCount : kindCounts[p.key],
  }));

  const liftClass = panelOpen ? "is-lifted" : "";
  // Translucent card body with a heavy backdrop blur — frosted-glass effect.
  // Selected state uses a primary-tinted overlay over the same base.
  const selectedBodyClass = selected
    ? "border-primary/45 bg-primary/20"
    : "border-border bg-card/40";

  return (
    <div
      onMouseEnter={() => setShowCheckbox(true)}
      onMouseLeave={() => setShowCheckbox(false)}
      onFocus={() => setShowCheckbox(true)}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node)) {
          setShowCheckbox(false);
        }
      }}
      className={`folder-card group relative aspect-[1.25/1] w-full isolate ${liftClass}`}
    >
      {visiblePapers.map((p, idx) => {
        const t = TILTS[idx] ?? TILTS[TILTS.length - 1];
        return (
          <div
            key={p.key}
            data-paper={p.key}
            className={`folder-paper ${p.tint} absolute left-[12%] right-[12%] top-[8%] h-[38%] rounded-[14px] shadow-sm transition-transform duration-[520ms] [transition-timing-function:cubic-bezier(.22,.61,.36,1)]`}
            style={{
              transform: `translate(${t.x}, ${t.y}) rotate(${t.rot})`,
              zIndex: visiblePapers.length - idx,
            }}
          />
        );
      })}

      {/* Folder body — name on top + per-kind pills below */}
      <button
        type="button"
        onClick={onOpenDetail}
        title={name}
        className={`folder-body absolute inset-x-0 bottom-0 top-[20%] z-[4] flex cursor-pointer flex-col items-center justify-start gap-1.5 overflow-hidden rounded-[14px] border px-3 py-2 text-center shadow-md backdrop-blur-md [transition:transform_520ms_cubic-bezier(.22,.61,.36,1),box-shadow_520ms_cubic-bezier(.22,.61,.36,1),background-color_200ms,border-color_200ms] ${selectedBodyClass}`}
      >
        <span className="block w-full truncate text-lg font-semibold leading-tight">
          {name}
        </span>
        <div className="flex flex-wrap items-start justify-center gap-1">
          {pillEntries.map((p) => (
            <span
              key={p.key}
              className={`inline-flex w-fit items-center gap-1 rounded-full px-1.5 py-px text-[10px] font-medium ring-1 ring-inset tabular-nums ${p.pill}`}
            >
              <span>{p.label}</span>
              <span>·</span>
              <span>{p.count}</span>
            </span>
          ))}
        </div>
      </button>

      {/* Selection toggle. When selected, always rendered as a filled primary
          badge with a checkmark. When unselected, only rendered on hover/focus
          as a translucent outline circle that signals "click to select". */}
      {(selected || showCheckbox) && (
        // biome-ignore lint/a11y/useSemanticElements: custom <button role="checkbox"> needed for stopPropagation + custom icon rendering inside the interactive card.
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onToggleSelect();
          }}
          className={`absolute right-2 top-2 z-[5] flex h-5 w-5 items-center justify-center rounded-full border shadow-sm transition-colors ${
            selected
              ? "border-primary bg-primary text-primary-foreground"
              : "border-muted-foreground/40 bg-card/90 hover:border-primary hover:bg-card"
          }`}
          role="checkbox"
          aria-checked={selected}
          aria-label={t("actions.selectKit", { name })}
        >
          {selected && <Check className="h-3 w-3" strokeWidth={3} />}
        </button>
      )}
    </div>
  );
}
