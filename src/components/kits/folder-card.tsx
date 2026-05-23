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
// in this exact order. `tint` is the paper bg; `dot` is a small kind-colored
// circle inside the otherwise-neutral pill — kind color is conveyed by the
// dot + the paper behind, the pill chrome itself stays muted so the card
// doesn't read as multi-colored noise.
const PAPER_ORDER: Array<{
  key: keyof KindCounts | "config";
  tint: string;
  dot: string;
  label: string;
}> = [
  {
    key: "config",
    tint: "bg-paper-config",
    dot: "bg-muted-foreground/60",
    label: "FILE",
  },
  {
    key: "skill",
    tint: "bg-paper-skill",
    dot: "bg-kind-skill",
    label: "SKILL",
  },
  {
    key: "mcp",
    tint: "bg-paper-mcp",
    dot: "bg-kind-mcp",
    label: "MCP",
  },
];

// Slot assignment mirrors folders.html: 1 paper → center; 2 papers → left +
// right (no center); 3 papers → left + right + center (center sits on top via
// DOM order, since it's rendered last). Slot also drives hover transforms
// in index.css via `[data-slot]`.
const SLOTS_BY_COUNT: Record<
  number,
  Array<{
    x: string;
    y: string;
    rot: string;
    slot: "left" | "right" | "center";
  }>
> = {
  1: [{ x: "0%", y: "-2%", rot: "0deg", slot: "center" }],
  2: [
    { x: "-24%", y: "-8%", rot: "-4deg", slot: "left" },
    { x: "24%", y: "-6%", rot: "3deg", slot: "right" },
  ],
  3: [
    { x: "-24%", y: "-8%", rot: "-4deg", slot: "left" },
    { x: "24%", y: "-6%", rot: "3deg", slot: "right" },
    { x: "0%", y: "-2%", rot: "0deg", slot: "center" },
  ],
};

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
  // Dark mode uses a low-alpha white wash instead of bg-card/40 so the body
  // sits as a soft gray ABOVE page bg and lets the paper hues bleed through
  // the blur. Selected = highlighted border only (no bg tint, so the colored
  // papers behind still bleed through; an earlier primary-tinted bg washed
  // the card and hid the paper hues).
  const selectedBodyClass = selected
    ? "border-primary bg-card/40 dark:bg-white/[0.08]"
    : "border-border bg-card/40 dark:border-transparent dark:bg-white/[0.08]";

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
      {/* Render reversed so MCP (last in PAPER_ORDER) renders first and sits
          at the bottom of the stack; FILE (first in PAPER_ORDER) renders last
          and ends up on top. DOM order = paint order. */}
      {[...visiblePapers].reverse().map((p, idx) => {
        const slots = SLOTS_BY_COUNT[visiblePapers.length] ?? SLOTS_BY_COUNT[3];
        const s = slots[idx] ?? slots[slots.length - 1];
        return (
          <div
            key={p.key}
            data-paper={p.key}
            data-slot={s.slot}
            className={`folder-paper ${p.tint} absolute left-[16%] right-[16%] top-[14%] h-[46%] rounded-[18px] shadow-sm transition-transform duration-[520ms] [transition-timing-function:cubic-bezier(.22,.61,.36,1)]`}
            style={{
              transform: `translate(${s.x}, ${s.y}) rotate(${s.rot})`,
            }}
          />
        );
      })}

      {/* Folder body — name on top + per-kind pills below */}
      <button
        type="button"
        onClick={onOpenDetail}
        title={name}
        className={`folder-body absolute inset-x-0 bottom-0 top-[20%] z-[4] flex cursor-pointer flex-col items-center justify-center gap-1.5 overflow-hidden rounded-[18px] border px-3 py-2 text-center shadow-md backdrop-blur-md [transition:transform_520ms_cubic-bezier(.22,.61,.36,1),box-shadow_520ms_cubic-bezier(.22,.61,.36,1),background-color_200ms,border-color_200ms] ${selectedBodyClass}`}
      >
        <span className="block w-full truncate text-lg font-semibold leading-tight">
          {name}
        </span>
        <div className="flex flex-wrap items-start justify-center gap-1">
          {pillEntries.map((p) => (
            <span
              key={p.key}
              className="inline-flex w-fit items-center gap-1 rounded-full bg-muted/60 px-1.5 py-px text-[10px] font-medium text-muted-foreground ring-1 ring-inset ring-border/40 tabular-nums"
            >
              <span
                aria-hidden
                className={`h-1.5 w-1.5 shrink-0 rounded-full ${p.dot}`}
              />
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
