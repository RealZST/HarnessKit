import type { TFunction } from "i18next";
import { Download, FolderInput, Plus, Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import { FolderGrid } from "@/components/kits/folder-grid";
import { InstallDialog } from "@/components/kits/install-dialog";
import { KitDetailDrawer } from "@/components/kits/kit-detail-drawer";
import { KitEditorDialog } from "@/components/kits/kit-editor-dialog";
import { PathInputDialog } from "@/components/kits/path-input-dialog";
import { useKitStore } from "@/stores/kit-store";
import { useScopeStore } from "@/stores/scope-store";
import { toast } from "@/stores/toast-store";

const ONBOARDING_KEY = "hk:kits-v4:onboarding-toast-shown";

interface BatchInstallReq {
  kitIds: string[];
  projectPath?: string;
}

export default function KitsPage() {
  const { t } = useTranslation("kits");
  const kits = useKitStore((s) => s.kits);
  const installRecords = useKitStore((s) => s.installRecords);
  const fetchKits = useKitStore((s) => s.fetchKits);
  const fetchInstallRecords = useKitStore((s) => s.fetchInstallRecords);
  const fetchCandidates = useKitStore((s) => s.fetchCandidates);
  const importKit = useKitStore((s) => s.importKit);
  const scope = useScopeStore((s) => s.current);

  // Scope acts as a FILTER on the kit list (not a hard gate on actions):
  // - all: every kit
  // - project: kits installed in this project (via sync_targets)
  // - global: every kit (kits don't sync to global; no meaningful filter)
  // Actions (create / Add to Project / etc.) remain enabled in all scopes.
  const visibleKits = useMemo(() => {
    if (scope.type !== "project") return kits;
    const record = installRecords.find((r) => r.project_path === scope.path);
    if (!record) return [];
    const installed = new Set(record.entries.map((e) => e.kit_id));
    return kits.filter((k) => installed.has(k.id));
  }, [kits, installRecords, scope]);

  const [search, setSearch] = useState("");
  // Apply the header search filter on top of the scope-derived visible set.
  // `search_keywords` is a pre-built lowercased haystack assembled by the
  // backend (name + description + asset_names + config file_names), so a
  // single `includes` covers all match dimensions. Empty query = pass-through.
  const filteredKits = useMemo(() => {
    const lo = search.trim().toLowerCase();
    if (!lo) return visibleKits;
    return visibleKits.filter((k) => k.search_keywords.includes(lo));
  }, [visibleKits, search]);

  const [activeKitId, setActiveKitId] = useState<string | null>(null);
  const [selectedKitIds, setSelectedKitIds] = useState<string[]>([]);
  const [editorOpen, setEditorOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [batchInstall, setBatchInstall] = useState<BatchInstallReq | null>(
    null,
  );

  useEffect(() => {
    fetchKits().catch(console.error);
    fetchInstallRecords().catch(console.error);
  }, [fetchKits, fetchInstallRecords]);

  // Warm the editor's candidate list in the background — `list_kit_asset_candidates`
  // scans every extension + every agent config and probes them for on-disk
  // presence, which can take 1-2s. Deferring to after first paint keeps the
  // Kits page snappy on tab-click; KitEditorDialog still fetches on its own
  // mount, so opening "New Kit" before this finishes still works (the store
  // dedupes in-flight calls).
  useEffect(() => {
    const handle =
      typeof window.requestIdleCallback === "function"
        ? window.requestIdleCallback(() => {
            fetchCandidates().catch(console.error);
          })
        : window.setTimeout(() => {
            fetchCandidates().catch(console.error);
          }, 100);
    return () => {
      if (typeof window.cancelIdleCallback === "function") {
        window.cancelIdleCallback(handle as number);
      } else {
        window.clearTimeout(handle as number);
      }
    };
  }, [fetchCandidates]);

  // Mirror the Extensions page: when the user changes scope (Sidebar
  // ScopeSwitcher / etc.), collapse the detail drawer. The kit may not be
  // visible under the new scope filter, and a stale drawer pointing at an
  // off-list kit confuses the eye.
  const prevScopeRef = useRef(scope);
  useEffect(() => {
    if (prevScopeRef.current !== scope) {
      setActiveKitId(null);
      prevScopeRef.current = scope;
    }
  }, [scope]);

  // First-time onboarding toast: fires once when a user goes from 0 to ≥1 Kits.
  useEffect(() => {
    if (kits.length >= 1 && !localStorage.getItem(ONBOARDING_KEY)) {
      toast.info(t("toast.firstKitOnboarding"));
      localStorage.setItem(ONBOARDING_KEY, "1");
    }
  }, [kits.length, t]);

  function handleImport() {
    setImportOpen(true);
  }

  function handleApplySelected() {
    if (selectedKitIds.length === 0) return;
    // Project scope: prefill the install dialog with the current project so
    // the user lands on the conflict-preview step in one click (they can
    // still pick a different project inside the dialog).
    const prefillPath = scope.type === "project" ? scope.path : undefined;
    setBatchInstall({ kitIds: selectedKitIds, projectPath: prefillPath });
  }

  // Clearing selection on scope change avoids "I selected 3 kits in project A,
  // switched to project B, hit Apply, and the dialog opened with kits I can no
  // longer see" surprises.
  // biome-ignore lint/correctness/useExhaustiveDependencies: only react to scope flips, not selection edits.
  useEffect(() => {
    setSelectedKitIds([]);
  }, [scope]);

  const inSelectMode = selectedKitIds.length > 0;

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6 -mr-6">
      {/* Two-row header mirroring Extensions / Audit: title + primary actions
          inline on row 1; subtitle + search on row 2. `pr-6` compensates for
          the outer `-mr-6` so the right edge of the header content lands
          inside the normal padding (the grid/drawer below still extends to
          the page edge). */}
      <div className="shrink-0 space-y-3 border-b pb-4 pr-6">
        <div className="flex items-center gap-3">
          <h2 className="text-2xl font-bold tracking-tight select-none">
            {t("page.title")}
          </h2>
          <button
            type="button"
            onClick={() => setEditorOpen(true)}
            className="flex items-center gap-1 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/90 hover:shadow-md"
          >
            <Plus size={12} />
            {t("page.newKit")}
          </button>
          <button
            type="button"
            onClick={handleImport}
            className="flex items-center gap-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md"
          >
            <Download size={12} />
            {t("exportImport.import")}
          </button>
        </div>
        <div className="flex items-center gap-2">
          <p className="min-w-0 flex-1 truncate text-sm text-muted-foreground">
            {inSelectMode
              ? t("page.selectModeHint")
              : scope.type === "project"
                ? t("page.scopeHintProject", { name: scope.name })
                : scope.type === "global"
                  ? t("page.scopeHintGlobal")
                  : t("page.subtitle")}
          </p>
          {search && (
            <button
              type="button"
              onClick={() => setSearch("")}
              className="shrink-0 rounded-md bg-muted/60 px-2 py-0.5 text-xs text-foreground/70 transition-colors hover:bg-muted hover:text-foreground"
            >
              {t("page.clearFilters", { defaultValue: "Clear filters" })}
            </button>
          )}
          <div className="relative shrink-0 w-56">
            <Search
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
            />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t("page.searchPlaceholder", {
                defaultValue: "Search…",
              })}
              title={t("page.searchTitle", {
                defaultValue:
                  "Search by name, description, skill, MCP, or file name",
              })}
              aria-label={t("page.searchAria", {
                defaultValue: "Search kits",
              })}
              className="w-full rounded-lg border border-border bg-card py-1.5 pl-8 pr-8 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none"
            />
            {search && (
              <button
                type="button"
                onClick={() => setSearch("")}
                aria-label={t("page.clearSearch", {
                  defaultValue: "Clear search",
                })}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
              >
                <X size={14} />
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Relative container: holds the scrollable grid + the optional
          detail panel absolutely positioned on the right (matches the
          Extensions page pattern). */}
      <div className="relative flex-1 min-h-0">
        <div className="absolute inset-0 overflow-y-auto px-4 pt-6">
          {kits.length === 0 ? (
            <EmptyState
              onCreate={() => setEditorOpen(true)}
              onImport={handleImport}
              t={t}
            />
          ) : visibleKits.length === 0 && scope.type === "project" ? (
            <ScopeEmptyState
              projectName={scope.name}
              onSwitchToAll={() =>
                useScopeStore.getState().setScope({ type: "all" })
              }
              t={t}
            />
          ) : search.trim() !== "" && filteredKits.length === 0 ? (
            <SearchEmptyState onClear={() => setSearch("")} t={t} />
          ) : (
            <FolderGrid
              kits={filteredKits}
              activeKitId={activeKitId}
              selectedKitIds={selectedKitIds}
              onOpenDetail={setActiveKitId}
              onSelectionChange={setSelectedKitIds}
            />
          )}
        </div>
        {activeKitId && (
          <div className="absolute right-0 top-0 bottom-0 z-10 w-96">
            <KitDetailDrawer
              kitId={activeKitId}
              onClose={() => setActiveKitId(null)}
            />
          </div>
        )}

        {/* Floating selection toolbar — sticks to the bottom-center of the
            grid area when one or more Kits are selected. Lives outside the
            scrollable grid so it stays visible during scroll, and z-20 keeps
            it above the detail panel without competing with it. */}
        {inSelectMode && (
          <div
            role="toolbar"
            aria-label={t("page.selectModeHint")}
            className="absolute bottom-4 left-1/2 z-20 flex -translate-x-1/2 items-center gap-1.5 rounded-full border border-border bg-card px-2 py-1.5 shadow-lg"
          >
            <span className="px-2 text-xs font-medium text-muted-foreground tabular-nums">
              {t("page.nSelected", { count: selectedKitIds.length })}
            </span>
            <button
              type="button"
              onClick={handleApplySelected}
              className="inline-flex items-center gap-1 whitespace-nowrap rounded-full bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/90 hover:shadow-md"
            >
              <FolderInput size={12} />
              {t("actions.applySelected")}
            </button>
            <button
              type="button"
              onClick={() => setSelectedKitIds([])}
              className="inline-flex shrink-0 items-center rounded-full p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
              aria-label={t("common:cancel", "Cancel")}
            >
              <X size={14} />
            </button>
          </div>
        )}
      </div>

      {editorOpen && <KitEditorDialog onClose={() => setEditorOpen(false)} />}
      {importOpen && (
        <PathInputDialog
          title={t("exportImport.importTitle", { defaultValue: "Import Kit" })}
          description={t("exportImport.importDescription", {
            defaultValue:
              "Pick or paste the path to a Kit archive (.hk-kit.zip).",
          })}
          submitLabel={t("exportImport.import", { defaultValue: "Import" })}
          pickerMode="open"
          pickerFilters={[{ name: "HarnessKit Kit", extensions: ["zip"] }]}
          inputPlaceholder={t("exportImport.importPlaceholder", {
            defaultValue: "Paste a .hk-kit.zip file path…",
          })}
          inputHint={t("exportImport.importHint", {
            defaultValue: "Please select a .hk-kit.zip file.",
          })}
          onSubmit={async (p) => {
            if (!p.toLowerCase().endsWith(".hk-kit.zip")) {
              throw new Error(
                t("exportImport.importExtensionError", {
                  defaultValue:
                    "Please select a .hk-kit.zip file (the path must end with .hk-kit.zip).",
                }),
              );
            }
            const summary = await importKit(p);
            toast.success(
              t("exportImport.importSuccess", {
                name: summary.name,
                defaultValue: 'Imported kit "{{name}}"',
              }),
            );
          }}
          onClose={() => setImportOpen(false)}
        />
      )}
      {batchInstall && (
        <InstallDialog
          preFilledKitIds={batchInstall.kitIds}
          preFilledProjectPath={batchInstall.projectPath}
          onClose={() => {
            // Keep selectedKitIds intact on close so cancelling the dialog
            // doesn't reset the user's selection. Use the multi-select bar's
            // × button to clear selection explicitly.
            setBatchInstall(null);
          }}
          onInstalled={() => {
            // Successful install clears the selection — the user is "done"
            // with these kits for now, so the floating toolbar + selection
            // ring should retract on its own.
            setSelectedKitIds([]);
          }}
        />
      )}
    </div>
  );
}

function EmptyState({
  onCreate,
  onImport,
  t,
}: {
  onCreate(): void;
  onImport(): void;
  t: TFunction<"kits">;
}) {
  return (
    <div className="mx-auto flex max-w-md flex-col items-center gap-4 pt-24 text-center">
      <h2 className="text-lg font-semibold">{t("empty.title")}</h2>
      <p className="text-sm text-muted-foreground">{t("empty.subtitle")}</p>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onCreate}
          className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/90 hover:shadow-md"
        >
          <Plus size={12} />
          {t("page.newKit")}
        </button>
        <button
          type="button"
          onClick={onImport}
          className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md"
        >
          <Download size={12} />
          {t("exportImport.import")}
        </button>
      </div>
    </div>
  );
}

/** Empty state when the header search filter yields zero matches. Distinct
 *  from EmptyState (no kits at all) and ScopeEmptyState (none in this project
 *  scope) — kits exist, the query just doesn't match any of them. Styled to
 *  match Audit page's "no filter match" treatment for cross-page consistency. */
function SearchEmptyState({
  onClear,
  t,
}: {
  onClear(): void;
  t: TFunction<"kits">;
}) {
  return (
    <div className="py-8 text-center text-sm text-muted-foreground">
      {t("empty.noFilterMatch", {
        defaultValue: "No kits match your filters.",
      })}
      <button
        type="button"
        onClick={onClear}
        className="ml-1 font-medium text-foreground/70 transition-colors hover:text-foreground"
      >
        {t("page.clearFilters", { defaultValue: "Clear filters" })}
      </button>
    </div>
  );
}

/** Empty state when scope-filtering left zero kits in a project scope.
 *  Kits aren't created or installed from inside a project scope — you pick
 *  one in All scope and install it into a project. So the only meaningful
 *  CTA here is "switch back to All scope". */
function ScopeEmptyState({
  projectName,
  onSwitchToAll,
  t,
}: {
  projectName: string;
  onSwitchToAll(): void;
  t: TFunction<"kits">;
}) {
  return (
    <div className="mx-auto flex max-w-2xl flex-col items-center gap-4 pt-24 text-center">
      <h2 className="text-lg font-semibold">
        <Trans
          i18nKey="empty.scopeTitle"
          ns="kits"
          values={{ name: projectName }}
          components={{
            chip: (
              <span className="mx-0.5 rounded-md bg-muted px-2 py-0.5 font-mono text-base font-medium text-foreground" />
            ),
          }}
        />
      </h2>
      <p className="text-sm text-muted-foreground">
        {t("empty.scopeSubtitle")}
      </p>
      <button
        type="button"
        onClick={onSwitchToAll}
        className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/90 hover:shadow-md"
      >
        {t("empty.scopeSwitchToAll", {
          defaultValue: "Switch to All scope",
        })}
      </button>
    </div>
  );
}
