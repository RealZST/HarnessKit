import type { TFunction } from "i18next";
import { Download, FolderInput, Plus, X } from "lucide-react";
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
  const candidates = useKitStore((s) => s.candidates);
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
    // Prefetch Kit-editor candidates so opening "New Kit" doesn't trigger a
    // first-time IPC round-trip mid-render. Backend scan walks every
    // extension + every agent config and probes them for on-disk presence —
    // cheap to amortize on page load, noticeably janky if delayed until the
    // user clicks the editor button.
    if (!candidates) fetchCandidates().catch(console.error);
  }, [fetchKits, fetchInstallRecords, fetchCandidates, candidates]);

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
      <header className="flex shrink-0 items-center justify-between gap-4 border-b pb-4">
        <div className="min-w-0">
          <h1 className="text-2xl font-bold tracking-tight select-none">
            {t("page.title")}
          </h1>
          <p className="truncate text-sm text-muted-foreground">
            {inSelectMode
              ? t("page.selectModeHint")
              : scope.type === "project"
                ? t("page.scopeHintProject", { name: scope.name })
                : scope.type === "global"
                  ? t("page.scopeHintGlobal")
                  : t("page.subtitle")}
          </p>
        </div>
      </header>

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
          ) : (
            <FolderGrid
              kits={visibleKits}
              activeKitId={activeKitId}
              selectedKitIds={selectedKitIds}
              onOpenDetail={setActiveKitId}
              onSelectionChange={setSelectedKitIds}
              trailingChildren={
                !inSelectMode && (
                  <>
                    <GhostTile
                      icon={Plus}
                      label={t("page.newKit")}
                      onClick={() => setEditorOpen(true)}
                    />
                    <GhostTile
                      icon={Download}
                      label={t("exportImport.import")}
                      onClick={handleImport}
                    />
                  </>
                )
              }
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
        />
      )}
    </div>
  );
}

function GhostTile({
  icon: Icon,
  label,
  onClick,
}: {
  icon: typeof Plus;
  label: string;
  onClick(): void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="group/ghost flex aspect-[1.6/1] w-full cursor-pointer flex-col items-center justify-center gap-1.5 self-end rounded-[14px] border-2 border-dashed border-muted-foreground/30 bg-background/50 text-muted-foreground transition-colors hover:border-primary/60 hover:bg-primary/5 hover:text-primary"
    >
      <Icon className="h-5 w-5" strokeWidth={2} />
      <span className="text-xs font-medium">{label}</span>
    </button>
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
