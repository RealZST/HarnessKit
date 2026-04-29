import { useEffect, useState } from "react";

interface SectionAnchor {
  id: string;
  label: string;
}

/** Catalog of every anchor the rail knows about. The rail filters this list
 *  down to sections that are actually rendered in the current page. */
const SECTION_CATALOG: SectionAnchor[] = [
  { id: "section-settings", label: "Settings" },
  { id: "section-workflow", label: "Workflow" },
  { id: "section-rules", label: "Rules" },
  { id: "section-memory", label: "Memory" },
  { id: "section-ignore", label: "Ignore" },
  { id: "section-custom", label: "Custom" },
  { id: "section-extensions", label: "Extensions" },
];

/** Right-side fixed rail that jumps to a section on click and highlights the
 *  one currently in view. Hidden on narrow viewports.
 *
 *  Pass a `revisionKey` that changes whenever the visible sections might
 *  change (e.g. agent switched, scope filter toggled, custom paths added)
 *  so the rail re-discovers anchors and rewires the IntersectionObserver. */
export function SectionAnchorRail({ revisionKey }: { revisionKey: string }) {
  const [activeId, setActiveId] = useState<string | null>(null);
  const [presentIds, setPresentIds] = useState<Set<string>>(new Set());

  useEffect(() => {
    const present = new Set<string>();
    for (const section of SECTION_CATALOG) {
      if (document.getElementById(section.id)) present.add(section.id);
    }
    setPresentIds(present);
    if (present.size === 0) return;

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setActiveId(entry.target.id);
            return;
          }
        }
      },
      {
        // Trigger when the section enters the upper third of the viewport so
        // the active dot tracks what the user is reading, not what's about to
        // scroll off the bottom.
        rootMargin: "-15% 0px -70% 0px",
        threshold: 0,
      },
    );

    for (const id of present) {
      const el = document.getElementById(id);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  }, [revisionKey]);

  const sections = SECTION_CATALOG.filter((s) => presentIds.has(s.id));
  if (sections.length === 0) return null;

  const scrollTo = (id: string) => {
    document.getElementById(id)?.scrollIntoView({
      behavior: "smooth",
      block: "start",
    });
  };

  return (
    <div className="hidden md:flex fixed right-3 top-1/2 -translate-y-1/2 flex-col gap-2 z-30 pointer-events-none">
      {sections.map((s) => {
        const isActive = activeId === s.id;
        return (
          <button
            key={s.id}
            type="button"
            onClick={() => scrollTo(s.id)}
            aria-label={`Jump to ${s.label}`}
            className="group flex items-center gap-2 justify-end pointer-events-auto"
          >
            <span className="text-[10px] text-foreground opacity-0 group-hover:opacity-100 transition-opacity bg-card border border-border rounded px-1.5 py-0.5 whitespace-nowrap shadow-sm">
              {s.label}
            </span>
            <span
              className={
                "block rounded-full transition-all " +
                (isActive
                  ? "w-2.5 h-2.5 bg-primary"
                  : "w-1.5 h-1.5 bg-muted-foreground/40 group-hover:bg-muted-foreground/80")
              }
            />
          </button>
        );
      })}
    </div>
  );
}
