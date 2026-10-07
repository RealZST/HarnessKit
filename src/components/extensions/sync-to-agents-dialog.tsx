import { clsx } from "clsx";
import { Check, Loader2, X } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  buildSyncPairs,
  type SyncSkipReason,
} from "@/components/extensions/sync-pairs";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import { Modal } from "@/components/ui/modal";
import { parseError } from "@/lib/error-types";
import { api } from "@/lib/invoke";
import {
  agentDisplayName,
  type ConfigScope,
  type GroupedExtension,
  sortAgents,
} from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

const TARGET: ConfigScope = { type: "global" };
type ReasonSuffix =
  | "alreadyInstalled"
  | "hookUnsupported"
  | "globalHookBlocked"
  | "transportUnsupported"
  | "noSource";

const REASON_KEY: Record<SyncSkipReason, ReasonSuffix> = {
  "already-installed": "alreadyInstalled",
  "hook-unsupported": "hookUnsupported",
  "global-hook-blocked": "globalHookBlocked",
  "transport-unsupported": "transportUnsupported",
  "no-source": "noSource",
};

interface AttemptResult {
  groupKey: string;
  agent: string;
  status: "installed" | "failed";
  message?: string;
}

export function SyncToAgentsDialog({
  groups,
  onClose,
  onDone,
}: {
  groups: GroupedExtension[];
  onClose: () => void;
  onDone: () => void;
}) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const agents = useAgentStore((s) => s.agents);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const extensions = useExtensionStore((s) => s.extensions);

  const detected = useMemo(
    () =>
      sortAgents(
        agents.filter((a) => a.detected && a.enabled),
        agentOrder,
      ),
    [agents, agentOrder],
  );
  const [checked, setChecked] = useState<Set<string>>(
    () => new Set(detected.map((a) => a.name)),
  );
  const [running, setRunning] = useState(false);
  const [results, setResults] = useState<AttemptResult[] | null>(null);

  const targets = useMemo(
    () => detected.filter((a) => checked.has(a.name)),
    [detected, checked],
  );
  const { pairs, skipped } = useMemo(
    () => buildSyncPairs(groups, targets, extensions),
    [groups, targets, extensions],
  );
  const nameByKey = useMemo(
    () => new Map(groups.map((g) => [g.groupKey, g.name])),
    [groups],
  );

  const done = results !== null;
  const installedCount =
    results?.filter((r) => r.status === "installed").length ?? 0;

  function toggleAgent(name: string) {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  }

  async function run() {
    if (running || done) return;
    setRunning(true);
    // Bulk uses the default Hermes category for the whole run — no
    // per-pair picker (single-item detail keeps its picker).
    let hermesCategory: string | undefined;
    if (pairs.some((p) => p.isHermesSkill)) {
      const cats = await api.listHermesCategories().catch(() => []);
      hermesCategory = cats[0] ?? "local";
    }
    const out: AttemptResult[] = [];
    const store = useExtensionStore.getState();
    for (const pair of pairs) {
      try {
        await store.installToAgent(
          pair.sourceId,
          pair.targetAgent,
          TARGET,
          pair.isHermesSkill ? hermesCategory : undefined,
        );
        out.push({
          groupKey: pair.groupKey,
          agent: pair.targetAgent,
          status: "installed",
        });
      } catch (e) {
        out.push({
          groupKey: pair.groupKey,
          agent: pair.targetAgent,
          status: "failed",
          message: parseError(e).message,
        });
      }
    }
    setResults(out);
    setRunning(false);
    toast.success(
      t("sync.installed", {
        count: out.filter((r) => r.status === "installed").length,
      }),
    );
  }

  return (
    <Modal
      onClose={done ? onDone : onClose}
      busy={running}
      ariaLabel={t("sync.title")}
    >
      <div className="flex items-center justify-between border-b border-border px-5 py-4">
        <h2 className="text-sm font-semibold">{t("sync.title")}</h2>
        {!running && (
          <button
            type="button"
            onClick={done ? onDone : onClose}
            aria-label={t("sync.close")}
            className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            <X size={14} />
          </button>
        )}
      </div>

      <div className="space-y-4 overflow-y-auto px-5 py-4">
        {!done && (
          <>
            <p className="text-xs text-muted-foreground">
              {t("sync.globalNote")}
            </p>
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="block text-sm font-medium">
                  {t("sync.targetAgents")}
                </span>
                {detected.length > 1 && (
                  <button
                    type="button"
                    onClick={() =>
                      setChecked(
                        checked.size === detected.length
                          ? new Set()
                          : new Set(detected.map((a) => a.name)),
                      )
                    }
                    className="text-xs text-muted-foreground hover:text-foreground"
                  >
                    {t("newSkills.selectAll", { count: detected.length })}
                  </button>
                )}
              </div>
              {detected.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  {t("sync.noAgents")}
                </p>
              ) : (
                <div className="flex flex-wrap gap-2">
                  {detected.map((a) => {
                    const sel = checked.has(a.name);
                    return (
                      // biome-ignore lint/a11y/useSemanticElements: agent tiles are visually buttons but semantically toggle-able (multi-select); aria-pressed/role=checkbox keeps the multi-select semantics on a styled <button>.
                      <button
                        key={a.name}
                        type="button"
                        role="checkbox"
                        aria-checked={sel}
                        onClick={() => toggleAgent(a.name)}
                        className={clsx(
                          "flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-medium transition-[background-color,border-color] duration-150",
                          sel
                            ? "border-primary/40 bg-primary/20 text-foreground"
                            : "border-border bg-primary/10 text-foreground hover:bg-primary/20 hover:border-ring",
                        )}
                      >
                        <AgentMascot name={a.name} size={14} />
                        <span>{agentDisplayName(a.name)}</span>
                        {sel && (
                          <Check size={12} className="shrink-0 text-primary" />
                        )}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          </>
        )}

        {done && results && (
          <div className="space-y-2">
            <p className="text-xs text-muted-foreground">
              {t("sync.installed", { count: installedCount })}
            </p>
            <ul className="space-y-1" aria-live="polite">
              {results.map((r, i) => (
                <li
                  key={`${r.groupKey}:${r.agent}:${i}`}
                  className="flex items-baseline justify-between gap-2 rounded-lg border border-border px-3 py-1.5 text-xs"
                >
                  <span className="truncate font-medium">
                    {nameByKey.get(r.groupKey) ?? r.groupKey} →{" "}
                    {agentDisplayName(r.agent)}
                  </span>
                  {r.status === "installed" ? (
                    <span className="shrink-0 text-primary">✓</span>
                  ) : (
                    <span
                      className="shrink-0 text-destructive"
                      title={r.message}
                    >
                      {r.message ?? "failed"}
                    </span>
                  )}
                </li>
              ))}
              {skipped.map((s) => (
                <li
                  key={`skip:${s.groupKey}:${s.targetAgent}`}
                  className="flex items-baseline justify-between gap-2 rounded-lg border border-border px-3 py-1.5 text-xs text-muted-foreground"
                >
                  <span className="truncate font-medium">
                    {nameByKey.get(s.groupKey) ?? s.groupKey} →{" "}
                    {agentDisplayName(s.targetAgent)}
                  </span>
                  <span className="shrink-0">
                    {t(`sync.reason.${REASON_KEY[s.reason]}`)}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>

      <div className="flex items-center justify-end gap-2 border-t border-border px-5 py-3">
        {!done ? (
          <>
            <button
              type="button"
              onClick={onClose}
              disabled={running}
              className="rounded-lg px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground disabled:opacity-50"
            >
              {tc("actions.cancel")}
            </button>
            <button
              type="button"
              onClick={run}
              disabled={running || pairs.length === 0 || targets.length === 0}
              className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {running && <Loader2 size={12} className="animate-spin" />}
              {t("sync.start", { count: pairs.length })}
            </button>
          </>
        ) : (
          <button
            type="button"
            onClick={onDone}
            className="rounded-lg bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:bg-primary/90"
          >
            {t("sync.done")}
          </button>
        )}
      </div>
    </Modal>
  );
}
