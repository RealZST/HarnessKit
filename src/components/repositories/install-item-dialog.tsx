import { clsx } from "clsx";
import { X } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatedEllipsis } from "@/components/shared/animated-ellipsis";
import { HermesCategoryPicker } from "@/components/shared/hermes-category-picker";
import { ScopeTargetField } from "@/components/shared/scope-target-field";
import { Modal } from "@/components/ui/modal";
import { useScope } from "@/hooks/use-scope";
import { canInstallAtScope } from "@/lib/agent-capabilities";
import { humanizeError } from "@/lib/errors";
import { api } from "@/lib/invoke";
import {
  agentDisplayName,
  type ConfigScope,
  type RepositoryItem,
  sortAgents,
} from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { inScope, resolveInstallTargetScope } from "@/stores/extension-helpers";
import { useRepositoryStore } from "@/stores/repository-store";
import { toast } from "@/stores/toast-store";

/** The agents of the last install, offered again in the next one. */
let lastInstallAgents: string[] = [];

/** Pick the agents and scope one repository skill is installed to. */
export function InstallItemDialog({
  repoId,
  item,
  onClose,
}: {
  repoId: string;
  item: RepositoryItem;
  onClose: () => void;
}) {
  const { t } = useTranslation("repositories");
  const { t: tc } = useTranslation("common");
  const { t: tm } = useTranslation("marketplace");
  const { t: te } = useTranslation("extensions");
  const titleId = useId();
  const agents = useAgentStore((s) => s.agents);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const install = useRepositoryStore((s) => s.install);
  const { scope } = useScope();
  const [target, setTarget] = useState<ConfigScope | null>(() =>
    resolveInstallTargetScope(scope),
  );
  const detected = sortAgents(
    agents.filter((a) => a.detected && a.enabled),
    agentOrder,
  );
  // Starts from the last install; a lone agent needs no ticking.
  const [checked, setChecked] = useState<Set<string>>(
    () =>
      new Set(detected.length === 1 ? [detected[0].name] : lastInstallAgents),
  );
  const [hermesCategories, setHermesCategories] = useState<string[]>([]);
  const [hermesCategory, setHermesCategory] = useState("local");
  const [busy, setBusy] = useState(false);

  // An agent that already has this skill in the target scope is shown as
  // installed: installing again would overwrite that copy. Updates belong to
  // Check Updates on the Extensions page.
  const hasCopy = (agent: string) =>
    target != null &&
    inScope(item.installed, target).some((copy) => copy.agent === agent);
  const targetAgents = detected
    .filter(
      (a) =>
        checked.has(a.name) &&
        target &&
        canInstallAtScope(a, "skill", target) &&
        !hasCopy(a.name),
    )
    .map((a) => a.name);
  const hermesSelected = targetAgents.includes("hermes");

  useEffect(() => {
    if (!hermesSelected) return;
    api
      .listHermesCategories()
      .then((categories) => {
        setHermesCategories(categories);
        setHermesCategory((current) =>
          categories.includes(current) ? current : (categories[0] ?? "local"),
        );
      })
      .catch(() => {});
  }, [hermesSelected]);

  function toggleAgent(name: string) {
    setChecked((prev) => {
      const next = new Set(prev);
      if (!next.delete(name)) next.add(name);
      return next;
    });
  }

  async function submit() {
    if (!target || targetAgents.length === 0) return;
    setBusy(true);
    try {
      await install({
        repo_id: repoId,
        item_id: item.item_id,
        target_agents: targetAgents,
        target_scope: target,
        hermes_category: hermesSelected
          ? hermesCategory.trim() || "local"
          : undefined,
      });
      lastInstallAgents = targetAgents;
      toast.success(t("installed", { name: item.name }));
      onClose();
    } catch (e) {
      toast.error(humanizeError(e));
      setBusy(false);
    }
  }

  return (
    <Modal onClose={onClose} busy={busy} ariaLabelledBy={titleId}>
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <h2 id={titleId} className="text-base font-semibold">
          {t("installTitle", { name: item.name })}
        </h2>
        <button
          type="button"
          onClick={onClose}
          disabled={busy}
          aria-label={tc("actions.cancel")}
          className="rounded-md p-1 hover:bg-muted disabled:opacity-50"
        >
          <X className="h-4 w-4" />
        </button>
      </header>
      <div className="flex-1 space-y-4 overflow-auto px-4 py-3">
        {/* Scope first: which agents already have the skill depends on it. */}
        <ScopeTargetField value={target} onChange={setTarget} />
        <fieldset disabled={!target}>
          <legend className="text-xs text-muted-foreground">
            {te("install.installTo")}
            {!target && ` · ${tm("detail.selectScopeFirst")}`}
          </legend>
          {detected.length === 0 && (
            <p className="mt-1.5 text-xs text-muted-foreground">
              {te("bulkInstall.noAgents")}
            </p>
          )}
          <div className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1.5">
            {detected.map((a) => {
              const capable = !target || canInstallAtScope(a, "skill", target);
              const installed = hasCopy(a.name);
              return (
                <label
                  key={a.name}
                  title={
                    capable
                      ? undefined
                      : tm("detail.kindNotSupported", {
                          agent: agentDisplayName(a.name),
                        })
                  }
                  className={clsx(
                    "flex items-center gap-1.5 text-xs",
                    capable && target
                      ? "text-foreground"
                      : "text-muted-foreground/50",
                  )}
                >
                  <input
                    type="checkbox"
                    checked={installed || (capable && checked.has(a.name))}
                    onChange={() => toggleAgent(a.name)}
                    disabled={busy || !capable || installed}
                    className="rounded border-border accent-primary"
                  />
                  {agentDisplayName(a.name)}
                  {installed && (
                    <span className="text-muted-foreground">
                      {t("alreadyInstalled")}
                    </span>
                  )}
                </label>
              );
            })}
          </div>
        </fieldset>
        {hermesSelected && (
          <div className="space-y-1.5">
            <p className="text-xs text-muted-foreground">
              {tc("hermesCategory.choose")}
            </p>
            <HermesCategoryPicker
              categories={hermesCategories}
              value={hermesCategory}
              onChange={setHermesCategory}
              disabled={busy}
            />
          </div>
        )}
      </div>
      <footer className="flex justify-end gap-2 border-t border-border px-4 py-3">
        <button
          type="button"
          onClick={onClose}
          disabled={busy}
          className="rounded-md border border-border px-3 py-1.5 text-sm disabled:opacity-50"
        >
          {tc("actions.cancel")}
        </button>
        <button
          type="button"
          onClick={submit}
          disabled={busy || !target || targetAgents.length === 0}
          className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground disabled:opacity-50"
        >
          {busy ? (
            <>
              {te("install.installing")}
              <AnimatedEllipsis />
            </>
          ) : (
            tc("actions.install")
          )}
        </button>
      </footer>
    </Modal>
  );
}
