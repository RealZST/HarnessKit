import { Bot, Check, Loader2, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { HermesCategoryPicker } from "@/components/shared/hermes-category-picker";
import { ScopeTargetField } from "@/components/shared/scope-target-field";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { useScope } from "@/hooks/use-scope";
import { humanizeError } from "@/lib/errors";
import { api } from "@/lib/invoke";
import {
  agentDisplayName,
  type ConfigScope,
  type RepositoryItemWithInstallStatus,
  sortAgents,
} from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useRepositoryStore } from "@/stores/repository-store";
import { toast } from "@/stores/toast-store";

interface InstallRepoItemDialogProps {
  open: boolean;
  onClose: () => void;
  repoId: string;
  item: RepositoryItemWithInstallStatus | null;
}

export function InstallRepoItemDialog({
  open,
  onClose,
  repoId,
  item,
}: InstallRepoItemDialogProps) {
  const { t } = useTranslation("repositories");
  const installItem = useRepositoryStore((s) => s.installItem);
  const { agents, agentOrder } = useAgentStore();
  const { scope: currentScope } = useScope();

  const [targetScope, setTargetScope] = useState<ConfigScope>(
    currentScope.type === "all"
      ? { type: "global" }
      : (currentScope as ConfigScope),
  );
  const [selectedAgents, setSelectedAgents] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hermesCategories, setHermesCategories] = useState<string[]>([]);
  const [hermesCategory, setHermesCategory] = useState<string>("local");

  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef, open);

  const availableAgents = useMemo(
    () =>
      sortAgents(
        agents.filter((a) => a.detected && a.enabled),
        agentOrder,
      ),
    [agents, agentOrder],
  );

  const hermesSelected = selectedAgents.has("hermes");

  useEffect(() => {
    if (hermesSelected) {
      api
        .listHermesCategories()
        .then((cats) => {
          setHermesCategories(cats);
          if (cats.length > 0) {
            setHermesCategory((prev) => (cats.includes(prev) ? prev : cats[0]));
          }
        })
        .catch(() => {});
    }
  }, [hermesSelected]);

  useEffect(() => {
    if (open) {
      setError(null);
      setLoading(false);

      if (item?.installed_agents && item.installed_agents.length > 0) {
        // Preselect agents that currently have this item installed (if detected/enabled)
        const installedAgentNames = new Set(
          item.installed_agents
            .map((ia) => ia.agent)
            .filter((name) => availableAgents.some((a) => a.name === name)),
        );
        setSelectedAgents(
          installedAgentNames.size > 0
            ? installedAgentNames
            : availableAgents.length > 0
              ? new Set([availableAgents[0].name])
              : new Set(),
        );

        if (currentScope.type === "all") {
          setTargetScope(item.installed_agents[0].scope ?? { type: "global" });
        } else {
          setTargetScope(currentScope as ConfigScope);
        }
      } else {
        // Fresh install: preselect first detected agent
        if (availableAgents.length > 0) {
          setSelectedAgents(new Set([availableAgents[0].name]));
        } else {
          setSelectedAgents(new Set());
        }
        setTargetScope(
          currentScope.type === "all"
            ? { type: "global" }
            : (currentScope as ConfigScope),
        );
      }
    }
  }, [open, item, currentScope, availableAgents]);

  if (!open || !item) return null;

  const toggleAgent = (name: string) => {
    setSelectedAgents((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  };

  const handleInstall = async (e: React.FormEvent) => {
    e.preventDefault();
    if (selectedAgents.size === 0) return;

    setLoading(true);
    setError(null);

    try {
      await installItem(
        repoId,
        item.item_id,
        Array.from(selectedAgents),
        targetScope,
        hermesSelected ? hermesCategory : undefined,
      );
      toast.success(t("item.installed"));
      onClose();
    } catch (err: unknown) {
      setError(humanizeError(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="install-item-title"
        className="w-full max-w-md rounded-2xl border border-border bg-card p-6 shadow-2xl space-y-5"
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-primary/10 text-primary">
              <Bot size={18} />
            </div>
            <div>
              <h2
                id="install-item-title"
                className="text-base font-semibold text-card-foreground"
              >
                {t("installDialog.title", { name: item.name })}
              </h2>
              <p className="text-xs text-muted-foreground">
                {t("installDialog.description")}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="rounded-lg p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
          >
            <X size={18} />
          </button>
        </div>

        {error && (
          <div className="rounded-xl border border-destructive/20 bg-destructive/10 px-3.5 py-2.5 text-xs text-destructive">
            {error}
          </div>
        )}

        <form onSubmit={handleInstall} className="space-y-4">
          <div className="space-y-2">
            <label className="text-xs font-medium text-foreground">
              {t("installDialog.targetAgents")}
            </label>
            <div className="grid grid-cols-2 gap-2 max-h-48 overflow-y-auto">
              {availableAgents.map((agent) => {
                const isSelected = selectedAgents.has(agent.name);
                const isAlreadyInstalled = item.installed_agents.some(
                  (ia) =>
                    ia.agent === agent.name &&
                    JSON.stringify(ia.scope) === JSON.stringify(targetScope),
                );

                return (
                  <button
                    key={agent.name}
                    type="button"
                    onClick={() => toggleAgent(agent.name)}
                    className={`flex items-center justify-between rounded-xl border px-3 py-2 text-xs transition-colors text-left ${
                      isSelected
                        ? "border-primary bg-primary/10 text-foreground font-medium"
                        : "border-border bg-background text-muted-foreground hover:bg-accent hover:text-foreground"
                    }`}
                  >
                    <span className="truncate">
                      {agentDisplayName(agent.name)}
                    </span>
                    {isSelected && (
                      <Check
                        size={14}
                        className="text-primary shrink-0 ml-1.5"
                      />
                    )}
                    {!isSelected && isAlreadyInstalled && (
                      <span className="text-[10px] text-muted-foreground/60 shrink-0">
                        {t("item.installed")}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-foreground">
              {t("installDialog.scope")}
            </label>
            <ScopeTargetField
              value={targetScope}
              onChange={(s) => setTargetScope(s ?? { type: "global" })}
              alwaysPick={true}
            />
          </div>

          {hermesSelected && (
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-foreground">
                Hermes category
              </label>
              <HermesCategoryPicker
                categories={hermesCategories}
                value={hermesCategory}
                onChange={setHermesCategory}
                disabled={loading}
              />
            </div>
          )}

          <div className="flex items-center justify-end gap-2.5 pt-2">
            <button
              type="button"
              onClick={onClose}
              disabled={loading}
              className="rounded-xl border border-border px-4 py-2 text-xs font-medium text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
            >
              {t("cancel")}
            </button>
            <button
              type="submit"
              disabled={loading || selectedAgents.size === 0}
              className="flex items-center gap-2 rounded-xl bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
            >
              {loading ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  {t("installDialog.submitting")}
                </>
              ) : (
                t("installDialog.submit")
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
