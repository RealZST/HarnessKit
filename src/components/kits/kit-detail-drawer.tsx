import {
  Calendar,
  Edit,
  FolderInput,
  FolderOpen,
  Lock,
  MinusCircle,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import i18n from "@/lib/i18n";
import { agentDisplayName } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useKitStore } from "@/stores/kit-store";
import { useProjectStore } from "@/stores/project-store";
import { useScopeStore } from "@/stores/scope-store";
import { toast } from "@/stores/toast-store";
import type { KitExtensionRef } from "@/types/kits";
import { DeleteKitConfirm } from "./delete-kit-confirm";
import { FilePreviewModal } from "./file-preview-modal";
import { InstallDialog } from "./install-dialog";
import { KitEditorDialog } from "./kit-editor-dialog";
import { PathInputDialog } from "./path-input-dialog";
import { RemoveFromProjectDialog } from "./remove-from-project-dialog";

interface Props {
  kitId: string;
  onClose(): void;
}

// Module-scope constants: stable across renders, no need to recreate.
const KIND_ORDER: readonly string[] = ["skill", "mcp", "cli", "plugin", "hook"];
const KIND_LABEL: Record<string, string> = {
  skill: "SKILL",
  mcp: "MCP",
  cli: "CLI",
  plugin: "PLUGIN",
  hook: "HOOK",
};
const KIND_PILL: Record<string, string> = {
  skill: "bg-kind-skill/15 text-kind-skill ring-kind-skill/30",
  mcp: "bg-kind-mcp/15 text-kind-mcp ring-kind-mcp/30",
  cli: "bg-kind-cli/15 text-kind-cli ring-kind-cli/30",
  plugin: "bg-kind-plugin/15 text-kind-plugin ring-kind-plugin/30",
  hook: "bg-kind-hook/15 text-kind-hook ring-kind-hook/30",
};

// Per-category badge tints used in the FILES section. Same palette the New
// Kit editor (editor-configs-tab.tsx) uses for the row category badge —
// importing rather than re-defining would create a frontend cycle through
// editor-configs-tab, so we keep this short table in sync by convention.
const CATEGORY_BADGE_COLORS: Record<string, string> = {
  rules: "bg-foreground/15 text-foreground",
  memory: "bg-muted text-muted-foreground",
};

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(i18n.resolvedLanguage ?? "en", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function KitDetailDrawer({ kitId, onClose }: Props) {
  const { t } = useTranslation("kits");
  const details = useKitStore((s) => s.details);
  const fetchDetails = useKitStore((s) => s.fetchDetails);
  const exportKit = useKitStore((s) => s.exportKit);
  const deleteKit = useKitStore((s) => s.deleteKit);
  // Used only by the DeleteKitConfirm cascade below (alsoUninstall path).
  const unsyncKit = useKitStore((s) => s.unsyncKit);
  const scope = useScopeStore((s) => s.current);
  const projects = useProjectStore((s) => s.projects);
  const agentOrder = useAgentStore((s) => s.agentOrder);

  const [editorOpen, setEditorOpen] = useState(false);
  const [installState, setInstallState] = useState<{
    projectPath?: string;
  } | null>(null);
  const [removeFromOpen, setRemoveFromOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [previewPath, setPreviewPath] = useState<string | null>(null);

  useEffect(() => {
    fetchDetails(kitId).catch(console.error);
  }, [kitId, fetchDetails]);

  // Group extensions by kind with an ordered iteration. Renders one
  // subheader per kind (skills together, MCP together, etc.).
  const groupedExtensions = useMemo(() => {
    const groups = new Map<string, KitExtensionRef[]>();
    if (!details?.extensions) return groups;
    for (const ext of details.extensions) {
      if (!groups.has(ext.kind)) groups.set(ext.kind, []);
      groups.get(ext.kind)?.push(ext);
    }
    return groups;
  }, [details]);

  if (!details) return null;

  function handleExport() {
    if (!details) return;
    setExportOpen(true);
  }

  const noTargets = details.sync_targets.length === 0;

  return (
    <aside className="relative flex h-full flex-col rounded-xl border border-border bg-card shadow-sm">
      {/* Header — matches Extension detail panel: title + close, with created-at meta below */}
      <div className="shrink-0 flex items-start justify-between border-b border-border px-5 py-4">
        <div className="min-w-0">
          <h3 className="truncate text-lg font-semibold">
            {details.summary.name}
          </h3>
          <div
            className="mt-1 flex items-center gap-1.5 text-xs text-muted-foreground"
            title={new Date(details.summary.created_at).toLocaleString()}
          >
            <Calendar size={12} />
            <span className="truncate">
              {t("detail.createdAt", {
                time: formatDate(details.summary.created_at),
              })}
            </span>
          </div>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label={t("common:cancel", "Close")}
          className="shrink-0 rounded-lg p-2.5 text-muted-foreground hover:text-foreground"
        >
          <X size={18} />
        </button>
      </div>

      {/* Optional description block */}
      {details.summary.description && (
        <div className="border-b border-border px-5 py-3">
          <p className="text-sm text-muted-foreground">
            {details.summary.description}
          </p>
        </div>
      )}

      {/* Banner (corrupt only — Kits are immutable snapshots, no stale concept) */}
      {details.summary.corrupt && (
        <div className="border-b border-destructive/40 bg-destructive/10 px-5 py-2.5 text-sm text-destructive">
          {t("kitCorrupt.banner")}
        </div>
      )}

      {/* Scrollable body */}
      <div className="flex-1 space-y-5 overflow-auto px-5 py-4">
        {/* Primary action row — single CTA. The install dialog now handles
            both "add to existing" and "create new folder" via a mode switch,
            so the old separate "New Project with Kit" entry is redundant. */}
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() =>
              // Project scope: prefill the install dialog with the current
              // project — saves a click on the most common path. The user
              // can still pick a different project (or new folder) inside.
              setInstallState(
                scope.type === "project" ? { projectPath: scope.path } : {},
              )
            }
            disabled={details.summary.corrupt}
            className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/90 hover:shadow-md disabled:opacity-50"
          >
            <FolderInput size={12} />
            {t("actions.install")}
          </button>
          <button
            type="button"
            onClick={() => setRemoveFromOpen(true)}
            disabled={noTargets}
            className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-muted-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:text-foreground hover:shadow-md disabled:opacity-50"
          >
            <MinusCircle size={12} />
            {t("actions.removeFrom")}
          </button>
        </div>

        {/* One section per kind (Skills / MCP / CLI / Plugin / Hook). Each
            section renders its items as a chip rail — click a chip to jump to
            that extension's detail on the Extensions page. */}
        {KIND_ORDER.filter((k) => groupedExtensions.has(k)).map((kind) => {
          const items = groupedExtensions.get(kind) ?? [];
          return (
            <section key={kind}>
              <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {KIND_LABEL[kind]} ({items.length})
              </h4>
              <div className="flex flex-wrap gap-1.5">
                {items.map((ext) => (
                  <span
                    key={ext.extension_id}
                    title={ext.asset_name}
                    className={`inline-flex max-w-full items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ring-1 ring-inset ${KIND_PILL[kind]}`}
                  >
                    <span className="truncate">{ext.asset_name}</span>
                    {ext.secrets_stripped && (
                      <Lock
                        size={10}
                        className="shrink-0 opacity-70"
                        aria-label={t("exportImport.statusSecret")}
                      />
                    )}
                  </span>
                ))}
              </div>
            </section>
          );
        })}

        {/* FILES section — one card per source pick. Title = filename;
            right-side pill = category (Rules/Memory). Whole card opens the
            file preview modal. */}
        {details.config_files.length > 0 && (
          <section>
            <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {t("editor.configFiles")} ({details.config_files.length})
            </h4>
            <div className="space-y-2">
              {details.config_files.map((c, idx) => {
                const previewable = !!c.source_path;
                const categoryClass =
                  CATEGORY_BADGE_COLORS[c.category] ??
                  "bg-muted text-muted-foreground";
                return (
                  <button
                    // A Kit can carry multiple sources per (agent, category)
                    // — install-side merges them into one target file by
                    // concatenating contents. Index disambiguates the React
                    // key without leaking the source path into it.
                    key={`${c.agent}:${c.category}:${idx}`}
                    type="button"
                    onClick={() => {
                      if (c.source_path) setPreviewPath(c.source_path);
                    }}
                    disabled={!previewable}
                    title={c.source_path ?? c.source_file_name}
                    className="block w-full rounded-lg border border-border bg-card p-3 text-left enabled:hover:bg-accent disabled:cursor-default"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-sm font-medium">
                        {c.source_file_name}
                      </span>
                      <span
                        className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide ${categoryClass}`}
                      >
                        {c.category}
                      </span>
                    </div>
                    <div className="mt-1 flex items-start gap-2 text-muted-foreground">
                      <FolderOpen size={12} className="mt-0.5 shrink-0" />
                      <span className="break-all text-xs">
                        {c.source_path ?? "—"}
                      </span>
                    </div>
                  </button>
                );
              })}
            </div>
          </section>
        )}

        {/* INSTALLED IN section — one card per unique project, mirroring the
            Extension detail "PATHS" card layout. Header = project name +
            agent mascots (one per agent installed in this project); body =
            FolderOpen + full absolute path. */}
        {(() => {
          const byProject = new Map<string, string[]>();
          for (const tg of details.sync_targets) {
            const agents = byProject.get(tg.project_path) ?? [];
            if (!agents.includes(tg.agent_name)) agents.push(tg.agent_name);
            byProject.set(tg.project_path, agents);
          }
          // Sort each project's agents by the user-configurable agent order
          // from `useAgentStore` so mascots line up with the rest of the app
          // (Agents page, Extensions filters, install dialog, etc.).
          const agentRank = (name: string) => {
            const i = agentOrder.indexOf(name);
            return i === -1 ? Number.MAX_SAFE_INTEGER : i;
          };
          for (const agents of byProject.values()) {
            agents.sort((a, b) => agentRank(a) - agentRank(b));
          }
          const projectName = (path: string) => {
            const registered = projects.find((p) => p.path === path);
            if (registered) return registered.name;
            return path.split("/").filter(Boolean).pop() ?? path;
          };
          return (
            <section data-testid="section-installed-in">
              <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                {t("detail.installedInProject", { count: byProject.size })}
              </h4>
              {byProject.size === 0 ? (
                <p className="text-xs text-muted-foreground">
                  {t("detail.notInstalled")}
                </p>
              ) : (
                <div className="space-y-2">
                  <p className="text-xs text-muted-foreground">
                    {t("detail.editPropagationHint")}
                  </p>
                  {[...byProject.entries()].map(([path, agents]) => (
                    <div
                      key={path}
                      className="rounded-lg border border-border bg-card p-3"
                    >
                      <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
                        <span className="font-medium">{projectName(path)}</span>
                        <div className="flex items-center gap-1">
                          {agents.map((a) => (
                            <span
                              key={a}
                              title={agentDisplayName(a)}
                              className="inline-flex items-center"
                            >
                              <AgentMascot name={a} size={14} />
                            </span>
                          ))}
                        </div>
                      </div>
                      <div className="mt-1.5 flex items-start gap-2 text-muted-foreground">
                        <FolderOpen size={12} className="mt-0.5 shrink-0" />
                        <span className="break-all text-xs">{path}</span>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </section>
          );
        })()}

        {/* Secondary actions — small icon-button row, de-emphasized */}
        <div className="flex items-center gap-1 border-t border-border pt-3">
          <button
            type="button"
            onClick={() => setEditorOpen(true)}
            disabled={details.summary.corrupt}
            className="inline-flex min-h-[28px] items-center gap-1 rounded-md px-2 py-1.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-50"
          >
            <Edit size={12} />
            {t("actions.edit")}
          </button>
          <button
            type="button"
            onClick={handleExport}
            disabled={details.summary.corrupt}
            className="inline-flex min-h-[28px] items-center gap-1 rounded-md px-2 py-1.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-50"
          >
            <Upload size={12} />
            {t("exportImport.export")}
          </button>
          <button
            type="button"
            onClick={() => setDeleteOpen(true)}
            className="ml-auto inline-flex min-h-[28px] items-center gap-1 rounded-md px-2 py-1.5 text-xs text-destructive hover:bg-destructive/10"
          >
            <Trash2 size={12} />
            {t("actions.delete")}
          </button>
        </div>
      </div>

      {removeFromOpen && (
        <RemoveFromProjectDialog
          kitId={kitId}
          syncTargets={details.sync_targets}
          onClose={() => setRemoveFromOpen(false)}
        />
      )}
      {editorOpen && (
        <KitEditorDialog
          initial={details}
          onClose={() => setEditorOpen(false)}
        />
      )}
      {installState && (
        <InstallDialog
          preFilledKitIds={[kitId]}
          preFilledProjectPath={installState.projectPath}
          onClose={() => setInstallState(null)}
        />
      )}
      {previewPath && (
        <FilePreviewModal
          path={previewPath}
          onClose={() => setPreviewPath(null)}
        />
      )}
      {exportOpen && (
        <PathInputDialog
          title={t("exportImport.exportTitle", { defaultValue: "Export Kit" })}
          description={t("exportImport.exportDescription", {
            defaultValue: "Pick or paste a target folder for the Kit archive.",
          })}
          submitLabel={t("exportImport.export", { defaultValue: "Export" })}
          pickerMode="directory"
          inputPlaceholder={t("exportImport.exportPlaceholder", {
            defaultValue: "Paste the target folder path…",
          })}
          inputHint={t("exportImport.exportHint", {
            defaultValue:
              "The file will be saved as <name>.hk-kit.zip in this folder.",
          })}
          onSubmit={async (dir) => {
            const trimmed = dir.trim().replace(/\/+$/, "");
            const fullPath = `${trimmed}/${details.summary.name}.hk-kit.zip`;
            await exportKit(kitId, fullPath);
            toast.success(
              t("exportImport.exportSuccess", {
                path: fullPath,
                defaultValue: "Exported to {{path}}",
              }),
            );
          }}
          onClose={() => setExportOpen(false)}
        />
      )}
      {deleteOpen && (
        <DeleteKitConfirm
          name={details.summary.name}
          syncTargets={details.sync_targets}
          onCancel={() => setDeleteOpen(false)}
          onConfirm={async (alsoUninstall) => {
            // Preserve existing cascade: when the user opts in, uninstall the
            // Kit from every target BEFORE deleting it. Each call is wrapped
            // because one bad target shouldn't block the rest of the cascade
            // or the final delete.
            if (alsoUninstall) {
              for (const target of details.sync_targets) {
                try {
                  await unsyncKit({
                    kit_id: kitId,
                    project_path: target.project_path,
                    agent_name: target.agent_name,
                  });
                } catch (e) {
                  console.error("uninstall failed during delete cascade", e);
                }
              }
            }
            await deleteKit(kitId);
            onClose();
          }}
        />
      )}
    </aside>
  );
}
