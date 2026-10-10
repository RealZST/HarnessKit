import { clsx } from "clsx";
import { Check, Loader2, X } from "lucide-react";
import { Fragment, useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { extensionDisplayName } from "@/components/extensions/extension-name";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import { KindBadge } from "@/components/shared/kind-badge";
import { ScopeTargetField } from "@/components/shared/scope-target-field";
import { Modal } from "@/components/ui/modal";
import {
  buildInstallPlan,
  groupBy,
  type InstallItemRef,
  type InstallPair,
  type InstallPlan,
  type InstallSkip,
  type InstallSkipReason,
} from "@/lib/install-plan";
import { api } from "@/lib/invoke";
import {
  type AgentInfo,
  agentDisplayName,
  type ConfigScope,
  type GroupedExtension,
  sortAgents,
} from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { resolveInstallTargetScope } from "@/stores/extension-helpers";
import { useExtensionStore } from "@/stores/extension-store";
import { useScopeStore } from "@/stores/scope-store";

const REASON_KEY = {
  "already-installed": "bulkInstall.reason.alreadyInstalled",
  "kind-unsupported": "bulkInstall.reason.kindUnsupported",
  "hook-unsupported": "bulkInstall.reason.hookUnsupported",
  "mcp-unsupported": "bulkInstall.reason.mcpUnsupported",
  "global-hook-blocked": "bulkInstall.reason.globalHookBlocked",
  "scope-unsupported": "bulkInstall.reason.scopeUnsupported",
  "transport-unsupported": "bulkInstall.reason.transportUnsupported",
  source: "bulkInstall.sourceCopy",
  duplicate: "bulkInstall.reason.includedElsewhere",
  "no-source": "bulkInstall.reason.noSource",
} as const satisfies Record<InstallSkipReason, string>;

/** Two-column layout of the summary's name / reason lines. */
const SUMMARY_GRID =
  "grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1.5 text-xs";

/** The holder of an overwrite's source isn't skipped in any sense the
 *  user cares about: its mascot is lit and it isn't counted. */
const isSource = (s: InstallSkip) => s.reason === "source";
const countSkipped = (plan: InstallPlan) =>
  plan.skipped.filter((s) => !isSource(s) && s.reason !== "duplicate").length;

/** A selected group whose every item is planned through other selected
 *  groups (a CLI bundle whose parts are selected on their own): those
 *  groups, or null. Such a group gets one line, not a row of agents. */
function coveredBy(plan: InstallPlan, groupKey: string): string[] | null {
  if (plan.pairs.some((p) => p.groupKey === groupKey)) return null;
  const skips = plan.skipped.filter(
    (s) => s.groupKey === groupKey && !isSource(s),
  );
  if (skips.length === 0 || !skips.every((s) => s.reason === "duplicate"))
    return null;
  return [...new Set(skips.flatMap((s) => s.via ?? []))];
}

interface AttemptResult {
  pair: InstallPair;
  error?: string;
}

/** A run in progress or finished. The plan is snapshotted when the run
 *  starts: after the closing refetch every success would re-plan as
 *  "already installed". */
interface Run extends InstallPlan {
  groups: GroupedExtension[];
  agents: AgentInfo[];
  results: AttemptResult[];
  done: boolean;
}

/** Bulk counterpart of the detail panel's "Install to Agent": every
 *  selected group goes to every chosen agent, at the picked target scope
 *  (defaults to the active scope; All mode has no default). During and
 *  after the run: one row per extension whose agent mascots light up as
 *  each copy lands, then a summary — failures, then what was skipped and
 *  why on demand. */
export function BulkInstallDialog({
  groups,
  onClose,
  onDone,
}: {
  groups: GroupedExtension[];
  onClose: () => void;
  /** Called from the results screen; the caller clears the selection. */
  onDone: () => void;
}) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const { t: tm } = useTranslation("marketplace");
  const agents = useAgentStore((s) => s.agents);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const extensions = useExtensionStore((s) => s.extensions);
  const installPairs = useExtensionStore((s) => s.installPairs);
  const scope = useScopeStore((s) => s.current);
  const agentsLabelId = useId();
  const skipListId = useId();

  // Same contract as the detail panel: the active scope is the default
  // target, the picker can point anywhere else, and All mode needs an
  // explicit pick before anything can be planned.
  const [pickedTarget, setPickedTarget] = useState<ConfigScope | null>(null);
  const target = useMemo(
    () => pickedTarget ?? resolveInstallTargetScope(scope),
    [scope, pickedTarget],
  );

  const detected = useMemo(
    () =>
      sortAgents(
        agents.filter((a) => a.detected && a.enabled),
        agentOrder,
      ),
    [agents, agentOrder],
  );
  // Nothing is pre-selected: with 15 agents, a pre-ticked list makes the
  // first click a 15-way install.
  const [checked, setChecked] = useState<Set<string>>(() => new Set());
  const [overwrite, setOverwrite] = useState(false);
  const [run, setRun] = useState<Run | null>(null);
  const [showSkipped, setShowSkipped] = useState(false);
  // Stop: the install in flight finishes, nothing after it is attempted.
  const stopRef = useRef(false);
  const [stopping, setStopping] = useState(false);

  // Every detected agent can be ticked; a tile says nothing about what
  // the agent would get. What was skipped and why is reported after the
  // run.
  const active = useMemo(
    () => (target ? detected.filter((a) => checked.has(a.name)) : []),
    [target, detected, checked],
  );
  // Frozen once a run starts: the placeholder rows landing would re-plan
  // every time.
  const plan = useMemo<InstallPlan>(
    () =>
      target && !run
        ? buildInstallPlan(groups, active, extensions, target, { overwrite })
        : { pairs: [], skipped: [] },
    [groups, active, extensions, target, overwrite, run],
  );

  function toggleAgent(name: string) {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  }

  async function start() {
    if (run || !target || plan.pairs.length === 0) return;
    setRun({ ...plan, groups, agents: active, results: [], done: false });
    // Bulk uses the default Hermes category for the whole run — no
    // per-pair picker (the single-item detail keeps its picker).
    let hermesCategory: string | undefined;
    if (plan.pairs.some((p) => p.isHermesSkill)) {
      const cats = await api.listHermesCategories().catch(() => []);
      hermesCategory = cats[0] ?? "local";
    }
    await installPairs(plan.pairs, target, {
      hermesCategory,
      onResult: (pair, error) =>
        setRun((r) => r && { ...r, results: [...r.results, { pair, error }] }),
      shouldStop: () => stopRef.current,
    });
    setRun((r) => r && { ...r, done: true });
  }

  // Keep the newest row in view as copies land, and end on the summary.
  const endRef = useRef<HTMLDivElement>(null);
  const landed = run?.results.length ?? 0;
  const runDone = run?.done ?? false;
  // biome-ignore lint/correctness/useExhaustiveDependencies: the two counters are the triggers; the body only touches the ref.
  useEffect(() => {
    if (!run) return;
    // Optional chaining: neither exists under jsdom.
    const reduced = window.matchMedia?.(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    endRef.current?.scrollIntoView?.({
      block: "nearest",
      behavior: reduced ? "auto" : "smooth",
    });
  }, [landed, runDone]);

  const running = run !== null && !run.done;
  const done = run?.done ?? false;
  const close = done ? onDone : onClose;
  const installedCount = run?.results.filter((r) => !r.error).length ?? 0;
  const failedCount = (run?.results.length ?? 0) - installedCount;
  const runSkipped = run ? countSkipped(run) : 0;
  const notRun = done && run ? run.pairs.length - run.results.length : 0;

  return (
    <Modal
      onClose={close}
      busy={running}
      ariaLabel={t("bulkInstall.title")}
      containerClassName="flex max-h-[90vh] w-[560px] flex-col rounded-xl border border-border bg-background shadow-xl"
    >
      <div className="flex items-center justify-between border-b border-border px-5 py-4">
        <h2 className="text-sm font-semibold">{t("bulkInstall.title")}</h2>
        {!running && (
          <button
            type="button"
            onClick={close}
            aria-label={tc("actions.close")}
            className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            <X size={14} />
          </button>
        )}
      </div>

      <div className="space-y-4 overflow-y-auto px-5 py-4">
        {!run && (
          <>
            <ScopeTargetField
              alwaysPick
              value={target}
              onChange={setPickedTarget}
            />
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span id={agentsLabelId} className="block text-sm font-medium">
                  {t("bulkInstall.targetAgents")}
                </span>
                {detected.length > 1 && (
                  <button
                    type="button"
                    disabled={!target}
                    onClick={() =>
                      setChecked(
                        active.length === detected.length
                          ? new Set()
                          : new Set(detected.map((a) => a.name)),
                      )
                    }
                    className="text-xs text-muted-foreground hover:text-foreground disabled:opacity-50"
                  >
                    {t("newSkills.selectAll", { count: detected.length })}
                  </button>
                )}
              </div>
              {detected.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  {t("bulkInstall.noAgents")}
                </p>
              ) : (
                <fieldset
                  aria-labelledby={agentsLabelId}
                  className="flex min-w-0 flex-wrap gap-1.5"
                >
                  {detected.map((a) => {
                    const sel = checked.has(a.name);
                    return (
                      // biome-ignore lint/a11y/useSemanticElements: agent tiles are visually buttons but semantically toggle-able (multi-select); role=checkbox keeps the multi-select semantics on a styled <button>.
                      <button
                        key={a.name}
                        type="button"
                        role="checkbox"
                        aria-checked={sel}
                        aria-label={agentDisplayName(a.name)}
                        disabled={!target}
                        title={
                          target ? undefined : tm("detail.selectScopeFirst")
                        }
                        onClick={() => toggleAgent(a.name)}
                        className={clsx(
                          "flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-medium transition-[background-color,border-color] duration-150 disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:border-border disabled:hover:bg-primary/10",
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
                </fieldset>
              )}
            </div>

            {plan.pairs.some((p) => p.isHermesSkill) && (
              <p className="text-xs text-muted-foreground">
                {t("bulkInstall.hermesNote")}
              </p>
            )}
          </>
        )}

        {run && (
          <div className="space-y-4">
            <div aria-live="polite">
              <div className="flex items-center justify-between text-xs text-muted-foreground">
                <span className="flex items-center gap-2">
                  {!done && <Loader2 size={12} className="animate-spin" />}
                  {done
                    ? notRun > 0
                      ? t("bulkInstall.stopped")
                      : t("bulkInstall.done")
                    : stopping
                      ? t("bulkInstall.stopping")
                      : t("bulkInstall.installing", {
                          done: run.results.length,
                          total: run.pairs.length,
                        })}
                </span>
                <span className="tabular-nums" aria-hidden="true">
                  {run.results.length} / {run.pairs.length}
                </span>
              </div>
              <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full bg-primary transition-[width] duration-300"
                  style={{
                    width: `${run.pairs.length ? (run.results.length / run.pairs.length) * 100 : 100}%`,
                  }}
                />
              </div>
            </div>

            <PlanRows
              groups={run.groups}
              agents={run.agents}
              plan={run}
              results={run.results}
            />

            {done && (
              <div
                className="animate-fade-in space-y-1 rounded-lg border border-border bg-muted/30 px-3 py-2"
                aria-live="polite"
              >
                <p className="flex items-center gap-2 text-sm">
                  <Check size={14} className="shrink-0 text-primary" />
                  <span className="font-medium">
                    {t("bulkInstall.installed", { count: installedCount })}
                  </span>
                  {failedCount > 0 && (
                    <span className="text-destructive">
                      · {t("bulkInstall.failed", { count: failedCount })}
                    </span>
                  )}
                  {notRun > 0 && (
                    <span className="text-muted-foreground">
                      · {t("bulkInstall.notRun", { count: notRun })}
                    </span>
                  )}
                  {runSkipped > 0 && (
                    <span className="text-muted-foreground">
                      ·{" "}
                      {/* Why something was left out matters once the run
                          is over, not before: the count opens the list. */}
                      <button
                        type="button"
                        onClick={() => setShowSkipped((v) => !v)}
                        aria-expanded={showSkipped}
                        aria-controls={skipListId}
                        className="underline decoration-dotted underline-offset-2 hover:text-foreground"
                      >
                        {t("bulkInstall.planSkipped", { count: runSkipped })}
                      </button>
                    </span>
                  )}
                </p>
                {failedCount > 0 && (
                  <ul className="space-y-1 pt-1 text-xs">
                    {run.results
                      .filter((r) => r.error)
                      .map((r) => (
                        <li
                          key={`${r.pair.sourceId}:${r.pair.targetAgent}`}
                          className="flex items-center gap-2"
                        >
                          <X size={12} className="shrink-0 text-destructive" />
                          <span className="flex shrink-0 items-center gap-1 font-medium">
                            <AgentMascot name={r.pair.targetAgent} size={12} />
                            {itemLabel(run.groups, r.pair)} →{" "}
                            {agentDisplayName(r.pair.targetAgent)}
                          </span>
                          <span
                            className="min-w-0 truncate text-destructive"
                            title={r.error}
                          >
                            {r.error}
                          </span>
                        </li>
                      ))}
                  </ul>
                )}
                {/* A bundle whose parts were selected on their own has no
                    row above. It leads the skipped list; with nothing else
                    skipped there is no list to open, so it shows as is. */}
                {runSkipped === 0 && (
                  <dl className={`${SUMMARY_GRID} pt-1`}>
                    <CoveredRows groups={run.groups} plan={run} />
                  </dl>
                )}
                {/* Failures first: they are what needs attention. */}
                {showSkipped && runSkipped > 0 && (
                  <div className="pt-1">
                    <SkipSummary
                      id={skipListId}
                      groups={run.groups}
                      plan={run}
                      agentCount={run.agents.length}
                    />
                  </div>
                )}
              </div>
            )}
            <div ref={endRef} />
          </div>
        )}
      </div>

      <div className="flex items-center justify-end gap-2 border-t border-border px-5 py-3">
        {done ? (
          <button
            type="button"
            onClick={onDone}
            className="rounded-lg bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:bg-primary/90"
          >
            {t("bulkInstall.done")}
          </button>
        ) : (
          <>
            {/* Modifies the commit, so it sits next to it: ticking it
                changes the count on the button right beside it. */}
            {!run && (
              <label
                className="mr-auto flex items-center gap-2 text-xs text-foreground"
                title={t("bulkInstall.overwriteHint")}
              >
                <input
                  type="checkbox"
                  checked={overwrite}
                  onChange={(e) => setOverwrite(e.target.checked)}
                  disabled={!target}
                  className="accent-primary"
                />
                {t("bulkInstall.overwrite")}
              </label>
            )}
            {running ? (
              <button
                type="button"
                onClick={() => {
                  stopRef.current = true;
                  setStopping(true);
                }}
                disabled={stopping}
                className="rounded-lg px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground disabled:opacity-50"
              >
                {stopping ? t("bulkInstall.stopping") : t("bulkInstall.stop")}
              </button>
            ) : (
              <button
                type="button"
                onClick={onClose}
                className="rounded-lg px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground"
              >
                {tc("actions.cancel")}
              </button>
            )}
            <button
              type="button"
              onClick={start}
              disabled={running || plan.pairs.length === 0}
              className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {running && <Loader2 size={12} className="animate-spin" />}
              {t("bulkInstall.start", {
                count: run
                  ? run.pairs.length - run.results.length
                  : plan.pairs.length,
              })}
            </button>
          </>
        )}
      </div>
    </Modal>
  );
}

/** "name" or "bundle / child" for anything the plan refers to. */
function itemLabel(groups: GroupedExtension[], ref: InstallItemRef): string {
  const g = groups.find((x) => x.groupKey === ref.groupKey);
  const name = g ? extensionDisplayName(g.kind, g.name) : ref.groupKey;
  return ref.itemName ? `${name} / ${ref.itemName}` : name;
}

/** The run view: one row per selected extension, one mascot per chosen
 *  agent. A mascot lights up when its copy lands (or, under Overwrite,
 *  when the agent holds the newest copy everyone else is copied from),
 *  stays dimmed while pending or skipped, and gets a red dot on failure.
 *  Reasons and errors are on hover. */
function PlanRows({
  groups,
  agents,
  plan,
  results,
}: {
  groups: GroupedExtension[];
  agents: AgentInfo[];
  plan: InstallPlan;
  results: AttemptResult[];
}) {
  const { t } = useTranslation("extensions");
  return (
    <ul className="space-y-1">
      {groups.map((g) => {
        if (coveredBy(plan, g.groupKey)) return null;
        const pairs = plan.pairs.filter((p) => p.groupKey === g.groupKey);
        const skips = plan.skipped.filter((s) => s.groupKey === g.groupKey);
        return (
          <li
            key={g.groupKey}
            className="flex items-center gap-2 rounded-lg border border-border px-3 py-1.5 text-xs"
          >
            <span
              className="min-w-[6rem] max-w-[14rem] truncate font-medium"
              title={g.name}
            >
              {extensionDisplayName(g.kind, g.name)}
            </span>
            <KindBadge kind={g.kind} noTooltip />
            <span className="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-1 text-muted-foreground">
              {agents.map((a) => {
                const mine = pairs.filter((p) => p.targetAgent === a.name);
                const mySkips = skips.filter((s) => s.targetAgent === a.name);
                const landed = results.filter((r) => mine.includes(r.pair));
                const failed = landed.find((r) => r.error);
                const lit =
                  landed.some((r) => !r.error) || mySkips.some(isSource);
                const title = [
                  agentDisplayName(a.name),
                  failed?.error,
                  ...mySkips.map((s) =>
                    s.itemName
                      ? `${s.itemName}: ${t(REASON_KEY[s.reason])}`
                      : t(REASON_KEY[s.reason]),
                  ),
                ]
                  .filter(Boolean)
                  .join(" · ");
                return (
                  <span
                    key={a.name}
                    title={title}
                    className={clsx(
                      "relative inline-flex rounded-full",
                      !lit && "opacity-30",
                      lit && "animate-scale-in",
                    )}
                  >
                    <AgentMascot name={a.name} size={14} />
                    {/* Same dot as the table's update marker, in red. */}
                    {failed && (
                      <span className="absolute -right-0.5 -top-0.5 h-2 w-2 rounded-full bg-destructive ring-1 ring-background" />
                    )}
                  </span>
                );
              })}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

/** What wasn't written, one line per extension: first any bundle
 *  covered by its separately selected parts, then the agents skipped,
 *  grouped by reason. The overwrite source isn't listed; a reason that
 *  covers every chosen agent is stated once, without a row of mascots. */
function SkipSummary({
  id,
  groups,
  plan,
  agentCount,
}: {
  id: string;
  groups: GroupedExtension[];
  plan: InstallPlan;
  agentCount: number;
}) {
  const { t } = useTranslation("extensions");
  // Same filter as `countSkipped`, so the list matches the count.
  const byItem = groupBy(
    plan.skipped.filter((s) => !isSource(s) && s.reason !== "duplicate"),
    (s) => `${s.groupKey}\0${s.itemName ?? ""}`,
  );
  return (
    <dl id={id} className={SUMMARY_GRID}>
      <CoveredRows groups={groups} plan={plan} />
      {[...byItem].map(([key, items]) => (
        <Fragment key={key}>
          <dt className="font-medium text-foreground">
            {itemLabel(groups, items[0])}
          </dt>
          <dd className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
            {[...groupBy(items, (s) => s.reason)].map(
              ([reason, ofReason], i) => (
                <span key={reason} className="inline-flex items-center gap-1">
                  {i > 0 && <span className="mr-1">·</span>}
                  {ofReason.length < agentCount &&
                    ofReason.map((s) => (
                      <span
                        key={s.targetAgent}
                        title={agentDisplayName(s.targetAgent)}
                      >
                        <AgentMascot name={s.targetAgent} size={12} />
                      </span>
                    ))}
                  <span className="text-muted-foreground">
                    {t(REASON_KEY[reason as InstallSkipReason])}
                  </span>
                </span>
              ),
            )}
          </dd>
        </Fragment>
      ))}
    </dl>
  );
}

/** One line per selected bundle whose parts were all selected on their
 *  own: it has no row in the run view, so this says where it went. Rows
 *  only; the caller provides the grid. */
function CoveredRows({
  groups,
  plan,
}: {
  groups: GroupedExtension[];
  plan: InstallPlan;
}) {
  const { t } = useTranslation("extensions");
  const groupName = (key: string) => {
    const g = groups.find((x) => x.groupKey === key);
    return g ? extensionDisplayName(g.kind, g.name) : key;
  };
  return groups.map((g) => {
    const via = coveredBy(plan, g.groupKey);
    if (!via) return null;
    return (
      <Fragment key={g.groupKey}>
        <dt className="font-medium text-foreground">{groupName(g.groupKey)}</dt>
        <dd className="text-muted-foreground">
          {t("bulkInstall.reason.coveredBy", {
            names: via.map(groupName).join(t("bulkInstall.nameSeparator")),
          })}
        </dd>
      </Fragment>
    );
  });
}
