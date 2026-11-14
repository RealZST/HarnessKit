import { FolderSearch } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatedEllipsis } from "@/components/shared/animated-ellipsis";
import { HermesCategoryPicker } from "@/components/shared/hermes-category-picker";
import { ScopeTargetField } from "@/components/shared/scope-target-field";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { useScope } from "@/hooks/use-scope";
import { canInstallAtScope } from "@/lib/agent-capabilities";
import { openDirectoryPicker } from "@/lib/dialog";
import { humanizeError } from "@/lib/errors";
import { api } from "@/lib/invoke";
import { isDesktop } from "@/lib/transport";
import type { ConfigScope } from "@/lib/types";
import { agentDisplayName, sortAgents } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

interface LocalInstallDialogProps {
  open: boolean;
  onClose: () => void;
  initialPath?: string;
  initialAgent?: string;
}

export function LocalInstallDialog({
  open,
  onClose,
  initialPath,
  initialAgent,
}: LocalInstallDialogProps) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const { t: tm } = useTranslation("marketplace");
  const [source, setSource] = useState(initialPath ?? "");
  const [selectedAgents, setSelectedAgents] = useState<Set<string>>(
    initialAgent ? new Set([initialAgent]) : new Set(),
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hermesCategories, setHermesCategories] = useState<string[]>([]);
  const [hermesCategory, setHermesCategory] = useState<string>("local");
  const fetch = useExtensionStore((s) => s.fetch);
  const { agents, fetch: fetchAgents, agentOrder } = useAgentStore();
  const { scope: currentScope } = useScope();
  const [installTargetScope, setInstallTargetScope] =
    useState<ConfigScope | null>(null);

  const dialogRef = useRef<HTMLDivElement>(null);
  const scanBtnRef = useRef<HTMLButtonElement>(null);
  useFocusTrap(dialogRef, open);

  const detectedAgents = sortAgents(
    agents.filter((a) => a.detected && a.enabled),
    agentOrder,
  );

  const hermesSelected = selectedAgents.has("hermes");

  useEffect(() => {
    if (hermesSelected) {
      api
        .listHermesCategories()
        .then((cats) => {
          setHermesCategories(cats);
          if (cats.length > 0 && !cats.includes(hermesCategory)) {
            setHermesCategory(cats[0]);
          }
        })
        .catch(() => {});
    }
  }, [hermesSelected, hermesCategory]);

  useEffect(() => {
    if (open) {
      fetchAgents();
      setSource(initialPath ?? "");
      setError(null);
      setLoading(false);
      setHermesCategory("local");

      if (initialAgent) {
        setSelectedAgents(new Set([initialAgent]));
      } else {
        const detected = agents.filter((a) => a.detected && a.enabled);
        setSelectedAgents(new Set(detected.map((a) => a.name)));
      }

      if (currentScope.type === "all") {
        setInstallTargetScope({ type: "global" });
      } else {
        setInstallTargetScope(currentScope as ConfigScope);
      }
    }
  }, [open, initialPath, initialAgent, agents, currentScope, fetchAgents]);

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

  const allAgentsSelected =
    detectedAgents.length > 0 &&
    detectedAgents.every((a) => selectedAgents.has(a.name));

  const toggleAllAgents = () => {
    if (allAgentsSelected) {
      setSelectedAgents(new Set());
    } else {
      setSelectedAgents(new Set(detectedAgents.map((a) => a.name)));
    }
  };

  const handleBrowse = async () => {
    const selected = await openDirectoryPicker({
      title: t("install.browseFolder"),
    });
    if (selected) setSource(selected);
  };

  const handleInstallAction = async () => {
    if (!source.trim() || selectedAgents.size === 0 || !installTargetScope) {
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const effectiveHermesCategory = hermesCategory.trim() || "local";
      const result = await api.installFromLocal(
        source.trim(),
        [...selectedAgents],
        installTargetScope,
        hermesSelected ? effectiveHermesCategory : undefined,
      );
      await fetch();
      onClose();
      toast.success(t("install.nameInstalled", { name: result.name }));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const title = t("install.titleLocal");
  const description = isDesktop()
    ? t("install.descLocalDesktop")
    : t("install.descLocalWeb");
  const placeholder = t("install.localPlaceholder");

  return (
    <div
      className="grid transition-[grid-template-rows] duration-[250ms]"
      style={{ gridTemplateRows: open ? "1fr" : "0fr" }}
    >
      <div className="overflow-hidden">
        <div
          ref={dialogRef}
          role="dialog"
          aria-modal="true"
          className="rounded-xl border border-border bg-card p-4 shadow-sm"
        >
          <h3 className="text-sm font-semibold">{title}</h3>
          <p className="mt-1 text-xs text-muted-foreground">{description}</p>
          <div className="mt-3 flex items-center gap-1.5">
            <input
              type="text"
              value={source}
              onChange={(e) => setSource(e.target.value)}
              onKeyDown={(e) =>
                e.key === "Enter" &&
                !e.nativeEvent.isComposing &&
                e.keyCode !== 229 &&
                !loading &&
                scanBtnRef.current?.click()
              }
              placeholder={placeholder}
              aria-label={t("install.localPathAria")}
              aria-required="true"
              aria-describedby={error ? "install-error" : undefined}
              className="flex-1 rounded-lg border border-border bg-muted px-3 py-2 text-sm outline-none focus:border-ring focus:ring-2 focus:ring-ring/50"
              disabled={loading}
            />
            {isDesktop() && (
              <button
                onClick={handleBrowse}
                disabled={loading}
                className="shrink-0 rounded-lg border border-border bg-muted p-2 text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-40"
                title={t("install.browseFolder")}
              >
                <FolderSearch size={16} />
              </button>
            )}
          </div>
          {detectedAgents.length > 1 && (
            <div className="mt-3">
              <span className="text-xs text-muted-foreground">
                {t("install.installTo")}
              </span>
              <div className="mt-1.5 flex flex-wrap items-center gap-x-4 gap-y-1.5">
                <label className="flex items-center gap-1.5 text-xs font-medium text-foreground">
                  <input
                    type="checkbox"
                    checked={allAgentsSelected}
                    onChange={toggleAllAgents}
                    disabled={loading}
                    className="rounded border-border accent-primary"
                  />
                  {t("install.allAgents")}
                </label>
                <span className="text-border">|</span>
                {detectedAgents.map((a) => {
                  const capableAtScope =
                    !installTargetScope ||
                    canInstallAtScope(a, "skill", installTargetScope);
                  return (
                    <label
                      key={a.name}
                      title={
                        capableAtScope
                          ? undefined
                          : tm("detail.kindNotSupported", {
                              agent: a.name,
                            })
                      }
                      className={`flex items-center gap-1.5 text-xs ${
                        capableAtScope
                          ? "text-foreground"
                          : "text-muted-foreground/50"
                      }`}
                    >
                      <input
                        type="checkbox"
                        checked={capableAtScope && selectedAgents.has(a.name)}
                        onChange={() => toggleAgent(a.name)}
                        disabled={loading || !capableAtScope}
                        className="rounded border-border accent-primary"
                      />
                      {agentDisplayName(a.name)}
                    </label>
                  );
                })}
              </div>
            </div>
          )}
          <div className="mt-3">
            <ScopeTargetField
              value={installTargetScope}
              onChange={setInstallTargetScope}
            />
          </div>

          {hermesSelected && (
            <div className="mt-3">
              <span className="text-xs text-muted-foreground">
                {tc("hermesCategory.label")}
              </span>
              <div className="mt-1.5">
                <HermesCategoryPicker
                  categories={hermesCategories}
                  value={hermesCategory}
                  onChange={setHermesCategory}
                  disabled={loading}
                />
              </div>
            </div>
          )}

          {error && (
            <div
              id="install-error"
              className="mt-2 rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
            >
              {humanizeError(error)}
            </div>
          )}
          <div className="mt-3 flex items-center gap-2">
            <button
              ref={scanBtnRef}
              onClick={handleInstallAction}
              disabled={
                loading ||
                !source.trim() ||
                selectedAgents.size === 0 ||
                !installTargetScope
              }
              className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {loading ? (
                <>
                  {t("install.installing")}
                  <AnimatedEllipsis />
                </>
              ) : (
                t("install.install")
              )}
            </button>
            <button
              onClick={onClose}
              disabled={loading}
              className="rounded-lg px-4 py-2 text-sm text-muted-foreground hover:text-foreground"
            >
              {tc("actions.cancel")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// Keep backwards-compat export
