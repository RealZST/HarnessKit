import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Modal } from "@/components/ui/modal";
import { openDirectoryPicker } from "@/lib/dialog";
import { api } from "@/lib/invoke";
import type { AgentInfo, Project } from "@/lib/types";
import { useKitStore } from "@/stores/kit-store";
import { toast } from "@/stores/toast-store";
import { ConfigStep } from "./install-steps/config-step";
import { KitStep } from "./install-steps/kit-step";
import { type PreviewEntry, PreviewStep } from "./install-steps/preview-step";

type Step = "kit" | "config" | "preview" | "install";

interface Props {
  preFilledKitIds?: string[];
  preFilledProjectPath?: string;
  preFilledAgents?: string[];
  forceOverwriteMode?: boolean;
  onClose: () => void;
  /** Fired after install completes with at least one successful pair.
   *  Distinct from `onClose` (which also fires on cancel / preview-error),
   *  so the parent can scope side effects like "clear selection" to the
   *  success case only. */
  onInstalled?: () => void;
}

interface PairResult {
  kit_id: string;
  agent_name: string;
  ok: boolean;
  installed_count: number;
  reason?: string;
}

function initialStep(props: Props): Step {
  if (
    props.forceOverwriteMode &&
    props.preFilledKitIds?.length &&
    props.preFilledProjectPath &&
    props.preFilledAgents?.length
  ) {
    return "install";
  }
  // Kits flow: pick kit(s) first (if not pre-filled), then a single screen
  // that combines project picker + agent picker.
  if (!props.preFilledKitIds?.length) return "kit";
  return "config";
}

export function InstallDialog(props: Props) {
  const {
    preFilledKitIds,
    preFilledProjectPath,
    preFilledAgents,
    forceOverwriteMode,
    onClose,
    onInstalled,
  } = props;
  const { t } = useTranslation("kits");
  const kits = useKitStore((s) => s.kits);
  const fetchKits = useKitStore((s) => s.fetchKits);
  const previewConflicts = useKitStore((s) => s.previewConflicts);
  const syncKit = useKitStore((s) => s.syncKit);

  const [step, setStep] = useState<Step>(() => initialStep(props));
  const [projects, setProjects] = useState<Project[]>([]);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [projectPath, setProjectPath] = useState<string>(
    preFilledProjectPath ?? "",
  );
  const [selectedKitIds, setSelectedKitIds] = useState<string[]>(
    preFilledKitIds ? [...preFilledKitIds] : [],
  );
  const [selectedAgents, setSelectedAgents] = useState<string[]>(
    preFilledAgents ? [...preFilledAgents] : [],
  );
  const [previews, setPreviews] = useState<PreviewEntry[]>([]);
  const [forceExtIds, setForceExtIds] = useState<Set<string>>(new Set());
  const [forceCfgKeys, setForceCfgKeys] = useState<Set<string>>(new Set());
  const [submitting, setSubmitting] = useState(false);
  const [previewing, setPreviewing] = useState(false);
  const installLaunchedRef = useRef(false);
  // Track mount state so async callbacks skip setState after unmount. MUST
  // reset to true on each mount — StrictMode/HMR run the cleanup once before
  // re-mount, which would otherwise leave the ref stuck false and silently
  // no-op every async branch.
  const isMountedRef = useRef(true);
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    api.listProjects().then(setProjects).catch(console.error);
    api.listAgents().then(setAgents).catch(console.error);
    if (!kits.length) fetchKits().catch(console.error);
  }, [kits.length, fetchKits]);

  const totalPairs = selectedKitIds.length * selectedAgents.length;
  const installLabel =
    totalPairs === 1
      ? t("installDialog.installButton")
      : t("installDialog.installAllButton");

  // Compute conflict preview across all pairs, then either skip preview or render it.
  async function runPreviewThenInstall() {
    if (!projectPath || !selectedKitIds.length || !selectedAgents.length)
      return;
    // Double-click guard: bail if a preview cascade or install is in flight.
    if (previewing || submitting) return;
    setPreviewing(true);
    try {
      const pairs = selectedKitIds.flatMap((kid) =>
        selectedAgents.map((a) => ({ kid, a })),
      );
      const results = await Promise.all(
        pairs.map(async ({ kid, a }) => ({
          kit_id: kid,
          agent_name: a,
          preview: await previewConflicts({
            kit_id: kid,
            project_path: projectPath,
            agent_name: a,
          }),
        })),
      );
      // Bail if unmounted mid-await.
      if (!isMountedRef.current) return;
      setPreviews(results);
      const hasConflicts = results.some(
        (p) =>
          p.preview.extension_conflicts.length > 0 ||
          p.preview.config_conflicts.length > 0,
      );
      if (forceOverwriteMode) {
        // Auto-overwrite ALL conflicts — Re-install path. Kick install off in
        // the background and close the dialog immediately; result is reported
        // via toast (same UX as no-conflict path below).
        const extIds = new Set<string>();
        const cfgKeys = new Set<string>();
        for (const { preview } of results) {
          for (const c of preview.extension_conflicts) {
            extIds.add(c.extension_id);
          }
          for (const c of preview.config_conflicts) {
            cfgKeys.add(`${c.agent}:${c.category}`);
          }
        }
        void runInstall(results, extIds, cfgKeys);
        onClose();
      } else if (!hasConflicts) {
        // No-conflict path: skip the redundant "Install / 无冲突 — 即将安装"
        // intermediate screen. Run install in background, close dialog now,
        // toast on completion.
        void runInstall(results, forceExtIds, forceCfgKeys);
        onClose();
      } else {
        setStep("preview");
      }
    } catch (e) {
      // Preview failure surfaces as a toast (matching the success path);
      // the dialog falls back to the prior step so the user can adjust.
      if (!isMountedRef.current) return;
      toast.error(
        e instanceof Error ? e.message : String(e ?? "Preview failed"),
      );
      onClose();
    } finally {
      if (isMountedRef.current) setPreviewing(false);
    }
  }

  async function runInstall(
    previewEntries: PreviewEntry[] = previews,
    forceExt: Set<string> = forceExtIds,
    forceCfg: Set<string> = forceCfgKeys,
  ) {
    if (submitting) return;
    setSubmitting(true);
    const pairs = previewEntries.length
      ? previewEntries.map((p) => ({ kid: p.kit_id, a: p.agent_name }))
      : selectedKitIds.flatMap((kid) =>
          selectedAgents.map((a) => ({ kid, a })),
        );
    // Best-effort loop (Q8): collect per-pair results so one failed pair
    // doesn't short-circuit the rest. A failed pair is reported in the
    // result list; the user re-triggers install manually for now.
    // Note: this loop intentionally does NOT bail on unmount so background
    // installs (no-conflict path closes the dialog immediately) finish all
    // pairs and surface a single complete toast — toast is a global sink.
    const results: PairResult[] = [];
    for (const { kid, a } of pairs) {
      try {
        const result = await syncKit({
          kit_id: kid,
          project_path: projectPath,
          agent_name: a,
          force_overwrite_extension_ids: [...forceExt],
          force_overwrite_config_keys: [...forceCfg],
        });
        results.push({
          kit_id: kid,
          agent_name: a,
          ok: true,
          installed_count: result.installed_count,
        });
      } catch (e) {
        results.push({
          kit_id: kid,
          agent_name: a,
          ok: false,
          installed_count: 0,
          reason: e instanceof Error ? e.message : String(e),
        });
      }
    }
    const succeeded = results.filter((r) => r.ok);
    const failed = results.filter((r) => !r.ok);
    const installed = succeeded.reduce((sum, r) => sum + r.installed_count, 0);
    const agentCount = new Set(succeeded.map((r) => r.agent_name)).size;
    if (failed.length === 0) {
      const itemText = t("toast.installedItems", { count: installed });
      const agentText = t("toast.installedAgents", { count: agentCount });
      toast.success(
        t("toast.installed", {
          itemText,
          agentText,
          defaultValue: `Installed ${itemText} across ${agentText}`,
        }),
      );
    } else if (succeeded.length === 0) {
      toast.error(failed[0]?.reason ?? "Install failed");
    } else {
      toast.warning(
        t("resultDialog.partialSummary", {
          succeeded: succeeded.length,
          failed: failed.length,
          defaultValue: `${succeeded.length} succeeded, ${failed.length} failed`,
        }),
      );
    }
    // Tell parent to clear selection (etc) when at least one pair succeeded.
    if (succeeded.length > 0 && onInstalled) {
      onInstalled();
    }
    // Guard state + close against unmount (no-op when caller already closed).
    if (isMountedRef.current) {
      setSubmitting(false);
      onClose();
    }
  }

  // Auto-trigger install when initialStep returns "install" — only possible
  // under forceOverwriteMode + all-pre-filled (Re-install path). Other paths
  // into step="install" already call runInstall explicitly.
  // biome-ignore lint/correctness/useExhaustiveDependencies: one-shot trigger; ref guard prevents re-runs.
  useEffect(() => {
    if (step !== "install" || installLaunchedRef.current) return;
    if (!forceOverwriteMode) return;
    if (!projectPath || !selectedKitIds.length || !selectedAgents.length)
      return;
    installLaunchedRef.current = true;
    void runPreviewThenInstall();
  }, [step]);

  async function pickFolder() {
    const dir = await openDirectoryPicker();
    if (dir) {
      setProjectPath(dir);
    }
  }

  function advanceFromKit() {
    if (!selectedKitIds.length) return;
    setStep("config");
  }

  function advanceFromConfig() {
    if (!projectPath || !selectedAgents.length) return;
    void runPreviewThenInstall();
  }

  return (
    <Modal
      onClose={onClose}
      busy={submitting || previewing}
      containerClassName="flex max-h-[90vh] w-[640px] flex-col rounded-xl border border-border bg-background shadow-xl"
    >
      <header className="flex items-center justify-between border-b px-4 py-3">
        <h2 className="text-base font-semibold">
          {step === "kit"
            ? t("installDialog.stepKitPick")
            : step === "config"
              ? t("installDialog.stepConfig")
              : step === "preview"
                ? t("installDialog.stepPreview")
                : t("installDialog.stepInstall")}
        </h2>
        <button
          type="button"
          onClick={onClose}
          aria-label={t("installDialog.cancel")}
          className="rounded-md p-1 hover:bg-muted"
        >
          <X className="h-4 w-4" />
        </button>
      </header>
      <div className="flex-1 space-y-3 overflow-auto px-4 py-3">
        {step === "kit" && (
          <KitStep
            kits={kits}
            selectedKitIds={selectedKitIds}
            setSelectedKitIds={setSelectedKitIds}
          />
        )}
        {step === "config" && (
          <ConfigStep
            projects={projects}
            agents={agents}
            projectPath={projectPath}
            setProjectPath={setProjectPath}
            selectedAgents={selectedAgents}
            setSelectedAgents={setSelectedAgents}
            onBrowseFolder={pickFolder}
            t={t}
          />
        )}
        {step === "preview" && (
          <PreviewStep
            previews={previews}
            kits={kits}
            forceExtIds={forceExtIds}
            setForceExtIds={setForceExtIds}
            forceCfgKeys={forceCfgKeys}
            setForceCfgKeys={setForceCfgKeys}
            t={t}
          />
        )}
        {step === "install" && (
          <p
            data-testid="install-step-body"
            className="text-sm text-muted-foreground"
          >
            {t("installDialog.noConflicts")}
          </p>
        )}
      </div>
      <footer className="flex justify-end gap-2 border-t px-4 py-3">
        <button
          type="button"
          onClick={onClose}
          className="rounded-md border px-3 py-1.5 text-sm"
        >
          {t("installDialog.cancel")}
        </button>
        {step === "kit" && (
          <button
            type="button"
            disabled={!selectedKitIds.length || previewing || submitting}
            onClick={advanceFromKit}
            className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground disabled:opacity-50"
          >
            →
          </button>
        )}
        {step === "config" && (
          <button
            type="button"
            disabled={
              !projectPath || !selectedAgents.length || previewing || submitting
            }
            onClick={advanceFromConfig}
            className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground disabled:opacity-50"
          >
            {installLabel}
          </button>
        )}
        {step === "preview" && (
          <button
            type="button"
            disabled={submitting || previewing}
            onClick={() => {
              setStep("install");
              void runInstall();
            }}
            className="rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground disabled:opacity-50"
          >
            {installLabel}
          </button>
        )}
      </footer>
    </Modal>
  );
}
