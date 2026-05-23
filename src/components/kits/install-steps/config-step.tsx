import { clsx } from "clsx";
import type { TFunction } from "i18next";
import { Check, FolderSearch } from "lucide-react";
import { useMemo, useState } from "react";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import { isDesktop } from "@/lib/transport";
import { type AgentInfo, agentDisplayName, type Project } from "@/lib/types";
import { isWeb as web, webSelectStyle } from "@/lib/web-select";

type TFn = TFunction<"kits">;
type ProjectMode = "existing" | "new";

interface Props {
  projects: Project[];
  agents: AgentInfo[];
  projectPath: string;
  setProjectPath: (path: string) => void;
  selectedAgents: string[];
  setSelectedAgents: (names: string[]) => void;
  onBrowseFolder: () => void;
  t: TFn;
}

/** Combined project picker + agent picker on a single screen. The project
 *  section radio-switches between "existing" (registered project dropdown)
 *  and "new" (typed folder path + Browse on desktop). New folders are
 *  auto-registered on first install. */
export function ConfigStep({
  projects,
  agents,
  projectPath,
  setProjectPath,
  selectedAgents,
  setSelectedAgents,
  onBrowseFolder,
  t,
}: Props) {
  const selectedAgentSet = useMemo(
    () => new Set(selectedAgents),
    [selectedAgents],
  );

  // Mode is derived from current path: matches an existing project → "existing",
  // freeform → "new". Switching modes wipes the path from the other side.
  const initialMode: ProjectMode =
    projectPath && projects.some((p) => p.path === projectPath)
      ? "existing"
      : projectPath
        ? "new"
        : "existing";
  const [mode, setMode] = useState<ProjectMode>(initialMode);

  return (
    <div className="space-y-5">
      <fieldset className="space-y-3">
        <legend className="text-sm font-medium">
          {t("installDialog.projectLabel")}
        </legend>

        <label className="flex items-start gap-2">
          <input
            type="radio"
            name="install-project-mode"
            value="existing"
            checked={mode === "existing"}
            onChange={() => {
              setMode("existing");
              if (!projects.some((p) => p.path === projectPath)) {
                setProjectPath("");
              }
            }}
            className="mt-1 shrink-0"
          />
          <div className="flex-1 space-y-1.5">
            <span className="block text-xs text-foreground">
              {t("installDialog.modeExisting")}
            </span>
            <select
              value={mode === "existing" ? projectPath : ""}
              onChange={(e) => setProjectPath(e.target.value)}
              disabled={mode !== "existing"}
              aria-label={t("installDialog.projectLabel")}
              style={webSelectStyle}
              className={clsx(
                "w-full overflow-hidden text-ellipsis border border-border bg-card px-3 text-xs text-foreground focus:border-ring focus:outline-none disabled:opacity-50",
                web ? "rounded-[6px] h-[32px]" : "rounded-lg py-2",
              )}
            >
              <option value="">{t("installDialog.selectProject")}</option>
              {projects.map((p) => (
                <option key={p.path} value={p.path}>
                  {p.name} ({p.path})
                </option>
              ))}
            </select>
          </div>
        </label>

        <label className="flex items-start gap-2">
          <input
            type="radio"
            name="install-project-mode"
            value="new"
            checked={mode === "new"}
            onChange={() => {
              setMode("new");
              if (projects.some((p) => p.path === projectPath)) {
                setProjectPath("");
              }
            }}
            className="mt-1 shrink-0"
          />
          <div className="flex-1 space-y-1.5">
            <span className="block text-xs text-foreground">
              {t("installDialog.modeNew")}
            </span>
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={mode === "new" ? projectPath : ""}
                onChange={(e) => setProjectPath(e.target.value)}
                disabled={mode !== "new"}
                placeholder={t("installDialog.newFolderPlaceholder")}
                aria-label={t("installDialog.modeNew")}
                className="flex-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none disabled:opacity-50"
              />
              {isDesktop() && (
                <button
                  type="button"
                  onClick={onBrowseFolder}
                  disabled={mode !== "new"}
                  title={t("installDialog.browseFolder")}
                  className="shrink-0 rounded-lg border border-border bg-card p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
                >
                  <FolderSearch size={14} />
                </button>
              )}
            </div>
            {mode === "new" && (
              <p className="text-xs text-muted-foreground">
                {t("installDialog.newFolderHint")}
              </p>
            )}
          </div>
        </label>
      </fieldset>

      <div className="space-y-2">
        <span className="block text-sm font-medium">
          {t("installDialog.agentLabel")}
        </span>
        <p className="text-xs text-muted-foreground">
          {t("installDialog.agentPickHelper")}
        </p>
        <div className="flex flex-wrap gap-2">
          {agents.map((a) => {
            const sel = selectedAgentSet.has(a.name);
            return (
              // biome-ignore lint/a11y/useSemanticElements: agent tiles are visually buttons but semantically toggle-able (multi-select); aria-pressed/role=checkbox keeps the multi-select semantics on a styled <button>.
              <button
                key={a.name}
                type="button"
                role="checkbox"
                aria-checked={sel}
                onClick={() => {
                  if (sel) {
                    setSelectedAgents(
                      selectedAgents.filter((n) => n !== a.name),
                    );
                  } else {
                    setSelectedAgents([...selectedAgents, a.name]);
                  }
                }}
                className={clsx(
                  "flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-medium transition-[background-color,border-color] duration-150",
                  sel
                    ? "border-primary/40 bg-primary/20 text-foreground"
                    : "border-border bg-primary/10 text-foreground hover:bg-primary/20 hover:border-ring",
                )}
              >
                <AgentMascot name={a.name} size={14} />
                <span>{agentDisplayName(a.name)}</span>
                {sel && <Check size={12} className="shrink-0 text-primary" />}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
