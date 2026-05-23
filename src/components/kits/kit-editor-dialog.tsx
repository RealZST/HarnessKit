import { X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Modal } from "@/components/ui/modal";
import { useKitStore } from "@/stores/kit-store";
import { toast } from "@/stores/toast-store";
import type { KitConfigFileRef, KitDetails } from "@/types/kits";
import { EditorAssetTab } from "./editor-asset-tab";
import { EditorConfigsTab } from "./editor-configs-tab";

const KIND_DOT: Record<string, string> = {
  skill: "bg-kind-skill",
  mcp: "bg-kind-mcp",
  cli: "bg-kind-cli",
  plugin: "bg-kind-plugin",
  hook: "bg-kind-hook",
};

// Theme-aware solid dots for config-file chips. Matches the rules/memory
// tint distinction used in the Files tab list rows.
const CONFIG_DOT: Record<string, string> = {
  rules: "bg-foreground/60",
  memory: "bg-muted-foreground/60",
};

// Chip-rail sort order: skills → MCPs → others, then alphabetical by name.
const KIND_RANK: Record<string, number> = {
  skill: 0,
  mcp: 1,
  cli: 2,
  plugin: 3,
  hook: 4,
};

// Files tab sort: agent → category (rules before memory) → file_name.
const CATEGORY_RANK: Record<string, number> = { rules: 0, memory: 1 };

interface Props {
  initial?: KitDetails;
  onClose(): void;
}

type TabId = "skills" | "mcp" | "configs";

export function KitEditorDialog({ initial, onClose }: Props) {
  const { t } = useTranslation("kits");
  const candidates = useKitStore((s) => s.candidates);
  const fetchCandidates = useKitStore((s) => s.fetchCandidates);
  const createKit = useKitStore((s) => s.createKit);
  const updateKit = useKitStore((s) => s.updateKit);

  const [name, setName] = useState(initial?.summary.name ?? "");
  const [description, setDescription] = useState(
    initial?.summary.description ?? "",
  );
  const [extensionIds, setExtensionIds] = useState<string[]>(
    initial?.extensions.map((e) => e.extension_id) ?? [],
  );
  const [configs, setConfigs] = useState<KitConfigFileRef[]>(
    initial?.config_files ?? [],
  );
  const [tab, setTab] = useState<TabId>("skills");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Refresh candidates on dialog open so changes made outside the page
  // (e.g. user installed a new skill via Marketplace, then came back to
  // edit a Kit) show up. The dialog renders the cached candidates first;
  // the force-refetch is deferred to idle so opening the dialog doesn't
  // block on the 1-2s backend scan (extension × agent disk probes). The
  // reconcile useEffect below drops any selected IDs that disappear from
  // the refreshed list.
  useEffect(() => {
    const refresh = () => {
      fetchCandidates({ force: true }).catch(console.error);
    };
    const handle =
      typeof window.requestIdleCallback === "function"
        ? window.requestIdleCallback(refresh)
        : window.setTimeout(refresh, 100);
    return () => {
      if (typeof window.cancelIdleCallback === "function") {
        window.cancelIdleCallback(handle as number);
      } else {
        window.clearTimeout(handle as number);
      }
    };
  }, [fetchCandidates]);

  // Reconcile selected extensionIds with the (Kit-able) candidate list as
  // soon as it arrives. The backend filters out rows whose source has
  // disappeared from disk (e.g. a marketplace skill the user uninstalled),
  // and if we left those IDs in our selection state, Save would re-pack
  // them and fail. Surfaces a toast so the silent drop is visible.
  useEffect(() => {
    if (!candidates) return;
    const candidateIds = new Set(candidates.extensions.map((e) => e.id));
    const kept = extensionIds.filter((id) => candidateIds.has(id));
    if (kept.length === extensionIds.length) return;
    const dropped = extensionIds.length - kept.length;
    setExtensionIds(kept);
    toast.warning(t("editor.staleExtensionsDropped", { count: dropped }));
  }, [candidates, extensionIds, t]);

  const canSave = name.trim().length > 0 && !submitting;

  async function handleSave() {
    if (!canSave) return;
    setSubmitting(true);
    setError(null);
    try {
      if (initial) {
        await updateKit({
          id: initial.summary.id,
          name: name.trim(),
          description: description.trim(),
          extension_ids: extensionIds,
          config_files: configs,
        });
      } else {
        await createKit({
          name: name.trim(),
          description: description.trim(),
          extension_ids: extensionIds,
          config_files: configs,
        });
      }
      onClose();
    } catch (e) {
      const raw = String(e);
      // Backend tags the duplicate-name case with `kit-name-exists:<name>`
      // (see check_kit_name_available in kits/service.rs) so we can swap
      // SQLite's "UNIQUE constraint failed: kits.name" for a localized
      // message the user can act on.
      setError(
        raw.includes("kit-name-exists:") ? t("error.duplicateName") : raw,
      );
    } finally {
      setSubmitting(false);
    }
  }

  const tabs = [
    { id: "skills", labelKey: "editor.tabs.skills" },
    { id: "mcp", labelKey: "editor.tabs.mcp" },
    { id: "configs", labelKey: "editor.tabs.configs" },
  ] as const satisfies ReadonlyArray<{ id: TabId; labelKey: string }>;

  // Resolve currently-selected extension ids to their full Extension rows so
  // the shared chip rail can render name + kind color.
  const selectedExtensions = useMemo(() => {
    const all = candidates?.extensions ?? [];
    const idx = new Map(all.map((e) => [e.id, e]));
    const resolved = extensionIds
      .map((id) => idx.get(id))
      .filter((e): e is NonNullable<typeof e> => Boolean(e));
    return [...resolved].sort((a, b) => {
      const ra = KIND_RANK[a.kind] ?? 99;
      const rb = KIND_RANK[b.kind] ?? 99;
      if (ra !== rb) return ra - rb;
      return a.name.localeCompare(b.name);
    });
  }, [candidates, extensionIds]);

  const sortedConfigs = useMemo(() => {
    return [...configs].sort((a, b) => {
      if (a.agent !== b.agent) return a.agent.localeCompare(b.agent);
      const ca = CATEGORY_RANK[a.category] ?? 99;
      const cb = CATEGORY_RANK[b.category] ?? 99;
      if (ca !== cb) return ca - cb;
      return a.source_file_name.localeCompare(b.source_file_name);
    });
  }, [configs]);

  const totalSelected = selectedExtensions.length + sortedConfigs.length;

  return (
    <Modal
      onClose={onClose}
      ariaLabelledBy="kit-editor-title"
      busy={submitting}
      containerClassName="flex flex-col rounded-xl border border-border bg-background shadow-xl"
    >
      <div
        className="flex flex-col"
        style={{
          width: "min(95vw, 960px)",
          height: "min(90vh, 720px)",
        }}
      >
        <header className="flex items-center justify-between border-b px-5 py-3">
          <h2 id="kit-editor-title" className="text-base font-semibold">
            {initial ? t("editor.editTitle") : t("page.newKit")}
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md p-1 hover:bg-muted"
            aria-label={t("common:close", "Close")}
          >
            <X className="h-4 w-4" />
          </button>
        </header>

        <div className="grid grid-cols-1 gap-4 border-b px-5 py-3 sm:grid-cols-2">
          <label className="block">
            <span className="mb-1 block text-sm font-medium">
              {t("editor.name")}
              <span className="ml-1.5 text-xs font-normal text-muted-foreground">
                {t("editor.required")}
              </span>
            </span>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("editor.namePlaceholder")}
              className="w-full rounded-lg border border-border bg-card px-3 py-1.5 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none"
            />
          </label>
          <label className="block">
            <span className="mb-1 block text-sm font-medium">
              {t("editor.description")}
              <span className="ml-1.5 text-xs font-normal text-muted-foreground">
                {t("editor.optional")}
              </span>
            </span>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t("editor.descriptionPlaceholder")}
              className="w-full rounded-lg border border-border bg-card px-3 py-1.5 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none"
            />
          </label>
        </div>

        {/* Shared chip rail — aggregates selections across all 3 tabs.
            Fixed max-height with internal scroll so heavy selections don't
            blow up the dialog layout. */}
        <div
          data-testid="chip-rail"
          className="flex max-h-24 flex-wrap items-center gap-1.5 overflow-y-auto border-b border-dashed bg-muted/30 px-5 py-2"
        >
          {totalSelected === 0 ? (
            <span className="text-xs text-muted-foreground">
              {t("editor.chipRailEmpty")}
            </span>
          ) : (
            <>
              {selectedExtensions.map((ext) => (
                <button
                  key={ext.id}
                  type="button"
                  onClick={() =>
                    setExtensionIds(extensionIds.filter((i) => i !== ext.id))
                  }
                  className="inline-flex items-center gap-1 rounded-full border bg-background px-2 py-0.5 text-xs hover:bg-muted"
                  aria-label={t("editor.chipRemoveAria", { name: ext.name })}
                >
                  <span
                    aria-hidden
                    className={`h-1.5 w-1.5 rounded-full ${
                      KIND_DOT[ext.kind] ?? "bg-muted-foreground"
                    }`}
                  />
                  <span>{ext.name}</span>
                  <X className="h-3 w-3" />
                </button>
              ))}
              {sortedConfigs.map((c) => (
                <button
                  key={`${c.agent}:${c.category}:${c.source_path ?? ""}`}
                  type="button"
                  onClick={() =>
                    setConfigs(
                      configs.filter(
                        (x) =>
                          !(
                            x.agent === c.agent &&
                            x.category === c.category &&
                            (x.source_path ?? "") === (c.source_path ?? "")
                          ),
                      ),
                    )
                  }
                  className="inline-flex items-center gap-1 rounded-full border bg-background px-2 py-0.5 text-xs hover:bg-muted"
                  aria-label={t("editor.chipRemoveAria", {
                    name: c.source_file_name,
                  })}
                >
                  <span
                    aria-hidden
                    className={`h-1.5 w-1.5 rounded-full ${
                      CONFIG_DOT[c.category] ?? "bg-muted-foreground"
                    }`}
                  />
                  <span className="font-medium">{c.agent}</span>
                  <span>/{c.source_file_name}</span>
                  <X className="h-3 w-3" />
                </button>
              ))}
            </>
          )}
        </div>

        <div role="tablist" className="flex gap-1 border-b px-5 pt-2">
          {tabs.map((tt) => (
            <button
              key={tt.id}
              type="button"
              role="tab"
              id={`kit-editor-tab-${tt.id}`}
              aria-selected={tab === tt.id}
              aria-controls={`kit-editor-panel-${tt.id}`}
              onClick={() => setTab(tt.id)}
              className={`rounded-t-md px-3 py-1.5 text-sm font-medium ${
                tab === tt.id
                  ? "bg-background text-foreground border-b-2 border-primary"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              {t(tt.labelKey)}
            </button>
          ))}
        </div>

        <div
          role="tabpanel"
          id={`kit-editor-panel-${tab}`}
          aria-labelledby={`kit-editor-tab-${tab}`}
          className="flex-1 overflow-hidden px-5 py-3"
        >
          {tab === "skills" && (
            <EditorAssetTab
              kindFilter="skill"
              selectedIds={extensionIds}
              onSelectionChange={setExtensionIds}
              candidates={candidates?.extensions ?? []}
            />
          )}
          {tab === "mcp" && (
            <EditorAssetTab
              kindFilter="mcp"
              selectedIds={extensionIds}
              onSelectionChange={setExtensionIds}
              candidates={candidates?.extensions ?? []}
            />
          )}
          {tab === "configs" && (
            <EditorConfigsTab
              selected={configs}
              onSelectionChange={setConfigs}
              candidates={candidates?.config_files ?? []}
            />
          )}
        </div>

        {error && (
          <p className="border-t border-destructive/30 bg-destructive/5 px-5 py-2 text-sm text-destructive">
            {error}
          </p>
        )}

        <footer className="flex items-center justify-between border-t px-5 py-3">
          <span className="text-xs text-muted-foreground">
            {t("editor.summaryCount", {
              skills: extensionIds.filter((id) =>
                (candidates?.extensions ?? []).some(
                  (e) => e.id === id && e.kind === "skill",
                ),
              ).length,
              mcp: extensionIds.filter((id) =>
                (candidates?.extensions ?? []).some(
                  (e) => e.id === id && e.kind === "mcp",
                ),
              ).length,
              configs: configs.length,
            })}
          </span>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border px-3 py-1.5 text-sm"
            >
              {t("common:cancel", "Cancel")}
            </button>
            <button
              type="button"
              disabled={!canSave}
              onClick={handleSave}
              className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground disabled:opacity-50"
            >
              {initial ? t("actions.save", "Save") : t("page.newKit")}
            </button>
          </div>
        </footer>
      </div>
    </Modal>
  );
}
