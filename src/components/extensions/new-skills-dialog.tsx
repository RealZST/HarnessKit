import { clsx } from "clsx";
import {
  ChevronDown,
  ChevronRight,
  Download,
  EyeOff,
  Loader2,
  Package,
  Search,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ScopeTargetField } from "@/components/shared/scope-target-field";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { api } from "@/lib/invoke";
import type { ConfigScope, NewRepoSkill } from "@/lib/types";
import { agentDisplayName, sortAgents } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

interface NewSkillsDialogProps {
  skills: NewRepoSkill[];
  onInstall: (
    url: string,
    skillIds: string[],
    targetAgents: string[],
    targetScope: ConfigScope,
  ) => Promise<void>;
  onDismiss: () => void;
  onClose: () => void;
}

function IndeterminateCheckbox({
  checked,
  indeterminate,
  onChange,
  className,
  "aria-label": ariaLabel,
}: {
  checked: boolean;
  indeterminate: boolean;
  onChange: () => void;
  className?: string;
  "aria-label"?: string;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) {
      ref.current.indeterminate = indeterminate;
    }
  }, [indeterminate]);

  return (
    <input
      ref={ref}
      type="checkbox"
      checked={checked}
      onChange={onChange}
      className={clsx(
        "rounded border-border accent-primary cursor-pointer",
        className,
      )}
      aria-label={ariaLabel}
    />
  );
}

export function NewSkillsDialog({
  skills,
  onInstall,
  onDismiss,
  onClose,
}: NewSkillsDialogProps) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const dlgRef = useRef<HTMLDivElement>(null);
  const [search, setSearch] = useState("");
  const [collapsedRepos, setCollapsedRepos] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<Set<string>>(
    new Set(skills.map((s) => `${s.repo_url}::${s.skill_id}`)),
  );
  const [selectedAgents, setSelectedAgents] = useState<Set<string>>(new Set());
  const [installing, setInstalling] = useState(false);
  const [ignoring, setIgnoring] = useState(false);
  // The dialog appears unexpectedly after Check Updates discovery, so the
  // active UI scope is not necessarily where the user wants these skills.
  // Always require an explicit pick (no implicit "use current scope").
  const [pickedScope, setPickedScope] = useState<ConfigScope | null>(null);

  const agents = useAgentStore((s) => s.agents);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const detectedAgents = sortAgents(
    agents.filter((a) => a.detected && a.enabled),
    agentOrder,
  );

  // If only one agent detected, auto-select it
  const singleAgentName =
    detectedAgents.length === 1 ? detectedAgents[0].name : null;
  useEffect(() => {
    if (singleAgentName) {
      setSelectedAgents(new Set([singleAgentName]));
    }
  }, [singleAgentName]);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onClose]);

  useFocusTrap(dlgRef, true);

  // Filter skills by search query
  const filteredSkills = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return skills;
    return skills.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        (s.description?.toLowerCase().includes(q) ?? false) ||
        s.repo_url.toLowerCase().includes(q) ||
        (s.pack?.toLowerCase().includes(q) ?? false),
    );
  }, [skills, search]);

  // Group filtered skills by repo
  const grouped = useMemo(() => {
    const map = new Map<
      string,
      { pack: string | null; skills: NewRepoSkill[] }
    >();
    for (const skill of filteredSkills) {
      const key = skill.repo_url;
      let entry = map.get(key);
      if (!entry) {
        entry = { pack: skill.pack, skills: [] };
        map.set(key, entry);
      }
      entry.skills.push(skill);
    }
    return map;
  }, [filteredSkills]);

  const toggleSkill = (key: string) => {
    const next = new Set(selected);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setSelected(next);
  };

  const toggleAgent = (name: string) => {
    const next = new Set(selectedAgents);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    setSelectedAgents(next);
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

  // Visible selection calculations
  const visibleKeys = useMemo(
    () => filteredSkills.map((s) => `${s.repo_url}::${s.skill_id}`),
    [filteredSkills],
  );
  const visibleSelectedCount = useMemo(
    () => visibleKeys.filter((k) => selected.has(k)).length,
    [visibleKeys, selected],
  );
  const allVisibleSkillsSelected =
    visibleKeys.length > 0 && visibleSelectedCount === visibleKeys.length;
  const someVisibleSkillsSelected =
    visibleSelectedCount > 0 && !allVisibleSkillsSelected;

  const toggleAllVisibleSkills = () => {
    const next = new Set(selected);
    if (allVisibleSkillsSelected) {
      for (const k of visibleKeys) next.delete(k);
    } else {
      for (const k of visibleKeys) next.add(k);
    }
    setSelected(next);
  };

  const toggleRepo = (url: string) => {
    const repoSkills = grouped.get(url)?.skills ?? [];
    const repoKeys = repoSkills.map((s) => `${s.repo_url}::${s.skill_id}`);
    const allInRepoSelected = repoKeys.every((k) => selected.has(k));
    const next = new Set(selected);
    if (allInRepoSelected) {
      for (const k of repoKeys) next.delete(k);
    } else {
      for (const k of repoKeys) next.add(k);
    }
    setSelected(next);
  };

  const toggleCollapseRepo = (url: string) => {
    setCollapsedRepos((prev) => {
      const next = new Set(prev);
      if (next.has(url)) next.delete(url);
      else next.add(url);
      return next;
    });
  };

  const handleInstall = async () => {
    if (!pickedScope) return;
    setInstalling(true);
    try {
      const targetAgents = [...selectedAgents];
      // Collect per-repo selections across all skills (even if filtered out by search)
      const allGrouped = new Map<string, NewRepoSkill[]>();
      for (const s of skills) {
        let list = allGrouped.get(s.repo_url);
        if (!list) {
          list = [];
          allGrouped.set(s.repo_url, list);
        }
        list.push(s);
      }

      for (const [url, repoSkills] of allGrouped) {
        const selectedSkills = repoSkills.filter((s) =>
          selected.has(`${s.repo_url}::${s.skill_id}`),
        );
        if (selectedSkills.length === 0) continue;
        const skillIds = selectedSkills.map((s) => s.skill_id);
        await onInstall(url, skillIds, targetAgents, pickedScope);
      }
      onClose();
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      toast.error(t("install.installFailed", { msg }));
    } finally {
      setInstalling(false);
    }
  };

  const handleIgnore = async () => {
    if (selected.size === 0) return;
    setIgnoring(true);
    try {
      const skillsToIgnore = skills.filter((s) =>
        selected.has(`${s.repo_url}::${s.skill_id}`),
      );
      for (const skill of skillsToIgnore) {
        await api.ignoreRepoSkill(skill.repo_url, skill.skill_id, skill.name);
      }
      const remaining = skills.filter(
        (s) => !selected.has(`${s.repo_url}::${s.skill_id}`),
      );
      setSelected(
        new Set(remaining.map((s) => `${s.repo_url}::${s.skill_id}`)),
      );
      useExtensionStore.setState({ newRepoSkills: remaining });
      toast.success(t("dialog.ignoredCount", { count: skillsToIgnore.length }));
      if (remaining.length === 0) {
        onClose();
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      toast.error(msg);
    } finally {
      setIgnoring(false);
    }
  };

  const selectedCount = selected.size;
  const canInstall =
    selectedCount > 0 && selectedAgents.size > 0 && pickedScope !== null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px]" />

      <div
        ref={dlgRef}
        role="dialog"
        aria-modal="true"
        aria-label={t("newSkills.aria")}
        tabIndex={-1}
        className="relative z-10 flex flex-col w-full max-w-lg max-h-[85vh] rounded-xl border border-border bg-card p-5 shadow-xl animate-fade-in outline-none"
      >
        {/* Header */}
        <div className="flex items-center justify-between gap-2 shrink-0 pb-3 border-b border-border">
          <div className="flex items-center gap-2 min-w-0">
            <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
              <Package size={16} />
            </span>
            <div className="min-w-0">
              <h3 className="text-sm font-semibold text-foreground truncate">
                {t("newSkills.title")}
              </h3>
              <p className="text-xs text-muted-foreground truncate">
                {t("newSkills.subtitle", { count: skills.length })}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="shrink-0 rounded-lg p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
            aria-label={tc("actions.cancel")}
          >
            <X size={16} />
          </button>
        </div>

        {/* Toolbar: Search & Select All */}
        <div className="shrink-0 py-3 space-y-2.5">
          <div className="relative">
            <Search
              size={14}
              className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground"
            />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t("newSkills.searchPlaceholder")}
              className="w-full rounded-lg border border-border bg-background pl-8 pr-8 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary"
            />
            {search && (
              <button
                type="button"
                onClick={() => setSearch("")}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                aria-label={tc("actions.clear")}
              >
                <X size={12} />
              </button>
            )}
          </div>
          <div className="flex items-center justify-between text-xs">
            <label className="flex items-center gap-1.5 font-medium text-foreground cursor-pointer select-none">
              <IndeterminateCheckbox
                checked={allVisibleSkillsSelected}
                indeterminate={someVisibleSkillsSelected}
                onChange={toggleAllVisibleSkills}
                aria-label={t("newSkills.selectAll", {
                  count: filteredSkills.length,
                })}
              />
              <span>
                {t("newSkills.selectAll", { count: filteredSkills.length })}
              </span>
            </label>
            {selectedCount > 0 && (
              <span className="text-[11px] text-muted-foreground">
                {t("newSkills.selectedCount", { count: selectedCount })}
              </span>
            )}
          </div>
        </div>

        {/* Scrollable Body: Grouped & Collapsible Skill List */}
        <div className="flex-1 overflow-y-auto min-h-0 space-y-3 pr-1 py-1">
          {filteredSkills.length === 0 ? (
            <div className="py-8 text-center text-xs text-muted-foreground">
              {t("newSkills.noMatches")}
            </div>
          ) : (
            [...grouped].map(([url, group]) => {
              const isCollapsed = collapsedRepos.has(url);
              const repoSkillKeys = group.skills.map(
                (s) => `${s.repo_url}::${s.skill_id}`,
              );
              const repoSelectedCount = repoSkillKeys.filter((k) =>
                selected.has(k),
              ).length;
              const isRepoAll =
                repoSkillKeys.length > 0 &&
                repoSelectedCount === repoSkillKeys.length;
              const isRepoPartial = repoSelectedCount > 0 && !isRepoAll;

              return (
                <div
                  key={url}
                  className="rounded-lg border border-border/70 bg-card/50 overflow-hidden"
                >
                  {/* Repo Header */}
                  <div className="flex items-center justify-between gap-2 px-3 py-2 bg-muted/40 hover:bg-muted/60 transition-colors">
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      <button
                        type="button"
                        onClick={() => toggleCollapseRepo(url)}
                        className="p-0.5 text-muted-foreground hover:text-foreground rounded transition-colors"
                        aria-label={
                          isCollapsed
                            ? t("newSkills.expandRepoAria", {
                                repo: group.pack ?? url,
                              })
                            : t("newSkills.collapseRepoAria", {
                                repo: group.pack ?? url,
                              })
                        }
                      >
                        {isCollapsed ? (
                          <ChevronRight size={14} />
                        ) : (
                          <ChevronDown size={14} />
                        )}
                      </button>
                      <IndeterminateCheckbox
                        checked={isRepoAll}
                        indeterminate={isRepoPartial}
                        onChange={() => toggleRepo(url)}
                        aria-label={t("newSkills.repoSelectAria", {
                          repo: group.pack ?? url,
                        })}
                      />
                      <span
                        onClick={() => toggleCollapseRepo(url)}
                        className="text-xs font-semibold text-foreground truncate cursor-pointer select-none"
                        title={url}
                      >
                        {group.pack ?? url}
                      </span>
                    </div>
                    <span className="shrink-0 text-[10px] font-medium text-muted-foreground bg-muted px-1.5 py-0.5 rounded-full border border-border/50">
                      {repoSelectedCount}/{group.skills.length}
                    </span>
                  </div>

                  {/* Skills Inside Repo */}
                  {!isCollapsed && (
                    <div className="p-2 space-y-1.5">
                      {group.skills.map((skill) => {
                        const key = `${skill.repo_url}::${skill.skill_id}`;
                        return (
                          <label
                            key={key}
                            className="flex items-start gap-2 rounded-lg border border-border/60 bg-background/50 px-2.5 py-2 cursor-pointer hover:bg-muted/40 hover:border-border transition-colors"
                          >
                            <input
                              type="checkbox"
                              checked={selected.has(key)}
                              onChange={() => toggleSkill(key)}
                              className="mt-0.5 shrink-0 rounded border-border accent-primary"
                            />
                            <div className="min-w-0 flex-1">
                              <span className="text-xs font-medium text-foreground">
                                {skill.name}
                              </span>
                              {skill.description && (
                                <p className="mt-0.5 text-[11px] text-muted-foreground line-clamp-2 leading-relaxed">
                                  {skill.description}
                                </p>
                              )}
                            </div>
                          </label>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>

        {/* Pinned Footer */}
        <div className="shrink-0 border-t border-border pt-3 mt-auto space-y-3">
          {/* Scope Picker */}
          <ScopeTargetField
            value={pickedScope}
            onChange={setPickedScope}
            alwaysPick
          />

          {/* Agent Selection */}
          {detectedAgents.length > 0 && (
            <div>
              <span className="text-xs text-muted-foreground">
                {t("newSkills.installTo")}
              </span>
              <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1">
                <label className="flex items-center gap-1.5 text-xs font-medium text-foreground cursor-pointer">
                  <input
                    type="checkbox"
                    checked={allAgentsSelected}
                    onChange={toggleAllAgents}
                    className="rounded border-border accent-primary"
                  />
                  {t("newSkills.allAgents")}
                </label>
                <span className="text-border">|</span>
                {detectedAgents.map((a) => (
                  <label
                    key={a.name}
                    className="flex items-center gap-1.5 text-xs text-foreground cursor-pointer"
                  >
                    <input
                      type="checkbox"
                      checked={selectedAgents.has(a.name)}
                      onChange={() => toggleAgent(a.name)}
                      className="rounded border-border accent-primary"
                    />
                    {agentDisplayName(a.name)}
                  </label>
                ))}
              </div>
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center gap-2 pt-1">
            <button
              type="button"
              onClick={handleInstall}
              disabled={!canInstall || installing}
              className="flex-1 flex items-center justify-center gap-1.5 rounded-lg bg-primary px-3 py-2 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
            >
              {installing ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Download size={12} />
              )}
              {installing
                ? t("newSkills.installing")
                : t("newSkills.installSelected", { count: selectedCount })}
            </button>
            <button
              type="button"
              onClick={handleIgnore}
              disabled={selected.size === 0 || ignoring || installing}
              className="flex-1 flex items-center justify-center gap-1.5 rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground hover:bg-muted/50 hover:text-foreground transition-colors disabled:opacity-50"
            >
              {ignoring ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <EyeOff size={12} />
              )}
              {t("dialog.ignoreSelected", { count: selectedCount })}
            </button>
          </div>
          <div className="flex justify-center">
            <button
              type="button"
              onClick={onDismiss}
              className="text-xs text-muted-foreground hover:text-foreground transition-colors py-0.5 hover:underline"
            >
              {t("newSkills.notNow")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
