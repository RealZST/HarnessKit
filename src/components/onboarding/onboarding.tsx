import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowRight,
  Check,
  ChevronRight,
  Database,
  File,
  Globe,
  Terminal,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { annotate } from "rough-notation";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import { useFocusTrap } from "@/hooks/use-focus-trap";

/* ══════════════════════════════════════════════════════
   Constants & Hooks
   ══════════════════════════════════════════════════════ */

const INTERACTIVE = "a, button, input, select, textarea, [role='button']";
const ONBOARDING_KEY = "hk-onboarding-completed";

export function useOnboarding() {
  const [show, setShow] = useState(
    () => localStorage.getItem(ONBOARDING_KEY) !== "done",
  );
  const complete = () => {
    localStorage.setItem(ONBOARDING_KEY, "done");
    setShow(false);
  };
  return { show, complete };
}

const shimmerStyle = {
  background:
    "linear-gradient(90deg, var(--primary), color-mix(in oklch, var(--primary) 45%, white), var(--primary))",
  backgroundSize: "200% auto",
  WebkitBackgroundClip: "text",
  WebkitTextFillColor: "transparent",
  backgroundClip: "text",
  animation: "text-shimmer 4s ease-in-out infinite",
} as const;

/* ══════════════════════════════════════════════════════
   Mock Data
   ══════════════════════════════════════════════════════ */

const PERM_ICONS = {
  filesystem: File,
  network: Globe,
  shell: Terminal,
  database: Database,
} as const;
type PermKey = keyof typeof PERM_ICONS;

const MOCK_EXTENSIONS = [
  {
    kind: "skill" as const,
    agents: ["claude", "cursor", "gemini"] as const,
    perms: ["filesystem", "shell"] as PermKey[],
    score: 95,
    enabled: true,
  },
  {
    kind: "mcp" as const,
    agents: ["claude", "codex", "copilot"] as const,
    perms: ["network", "database"] as PermKey[],
    score: 82,
    enabled: true,
  },
  {
    kind: "plugin" as const,
    agents: ["cursor", "copilot"] as const,
    perms: ["filesystem"] as PermKey[],
    score: 100,
    enabled: true,
  },
  {
    kind: "hook" as const,
    agents: ["claude", "codex", "antigravity"] as const,
    perms: ["filesystem", "shell"] as PermKey[],
    score: 88,
    enabled: true,
  },
  {
    kind: "cli" as const,
    agents: ["gemini", "copilot"] as const,
    perms: ["filesystem", "network", "shell"] as PermKey[],
    score: 67,
    enabled: true,
  },
];

const MOCK_AUDIT = [
  {
    name: "my-skill",
    score: 78,
    clean: false,
    findings: 2,
    rules: [
      {
        name: "Broad Permissions",
        severity: "High" as const,
        findings: [{ loc: "SKILL.md:24" }],
      },
      {
        name: "Supply Chain Risk",
        severity: "Medium" as const,
        findings: [{ loc: "package.json:7" }],
      },
    ],
  },
  {
    name: "my-mcp",
    score: 85,
    clean: false,
    findings: 1,
    rules: [
      {
        name: "Supply Chain Risk",
        severity: "Medium" as const,
        findings: [{ loc: "package.json:3" }],
      },
    ],
  },
  { name: "my-cli", score: 100, clean: true },
  { name: "my-plugin", score: 100, clean: true },
];

const KIND_COLORS: Record<string, string> = {
  skill: "var(--kind-skill)",
  mcp: "var(--kind-mcp)",
  plugin: "var(--kind-plugin)",
  hook: "var(--kind-hook)",
  cli: "var(--kind-cli)",
};

/* ══════════════════════════════════════════════════════
   Main Onboarding Component
   ══════════════════════════════════════════════════════ */

interface OnboardingProps {
  onComplete: () => void;
}

const TOTAL_STEPS = 3;

export function Onboarding({ onComplete }: OnboardingProps) {
  const [step, setStep] = useState(0);
  const [visible, setVisible] = useState(false);
  const [exiting, setExiting] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const [mouse, setMouse] = useState({ x: 0, y: 0 });
  const handleMouseMove = useCallback((e: MouseEvent) => {
    const x = (e.clientX / window.innerWidth - 0.5) * 2;
    const y = (e.clientY / window.innerHeight - 0.5) * 2;
    setMouse({ x, y });
  }, []);

  useEffect(() => {
    requestAnimationFrame(() => setVisible(true));
  }, []);

  useFocusTrap(containerRef, visible);

  useEffect(() => {
    const onMouseDown = (e: MouseEvent) => {
      if (e.button !== 0) return;
      const target = e.target as HTMLElement;
      if (target.closest(INTERACTIVE)) return;
      e.preventDefault();
      getCurrentWindow().startDragging();
    };
    document.addEventListener("mousedown", onMouseDown);
    window.addEventListener("mousemove", handleMouseMove);
    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("mousemove", handleMouseMove);
    };
  }, [handleMouseMove]);

  const show = visible && !exiting;
  const isLast = step === TOTAL_STEPS - 1;

  const handleNext = () => {
    if (isLast) {
      setExiting(true);
      setTimeout(onComplete, 400);
    } else {
      setStep((s) => s + 1);
    }
  };

  return (
    <div
      ref={containerRef}
      tabIndex={-1}
      className="fixed inset-0 z-[9999] flex flex-col items-center justify-center bg-background transition-opacity duration-300 overflow-hidden"
      style={{ opacity: show ? 1 : 0 }}
    >
      {/* Progress dots */}
      <div className="absolute top-8 left-1/2 -translate-x-1/2 flex gap-2">
        {Array.from({ length: TOTAL_STEPS }, (_, i) => (
          <button
            key={i}
            type="button"
            onClick={() => setStep(i)}
            className={`h-2 rounded-full transition-all duration-300 cursor-pointer ${
              i === step
                ? "w-8 bg-primary"
                : i < step
                  ? "w-2 bg-primary/40 hover:bg-primary/70 hover:w-4"
                  : "w-2 bg-border hover:bg-muted-foreground/40 hover:w-4"
            }`}
            aria-label={`Step ${i + 1}`}
          />
        ))}
      </div>

      {/* Content */}
      <div
        key={step}
        className="relative flex flex-col items-center px-8"
        style={{
          maxWidth: step === 0 ? 720 : 1140,
          width: "100%",
          transform: `translate(${mouse.x * -3}px, ${mouse.y * -3}px)`,
          transition: "transform 400ms cubic-bezier(0.22, 1, 0.36, 1)",
          animation:
            "entrance-spring 600ms cubic-bezier(0.34, 1.56, 0.64, 1) both",
        }}
      >
        {step === 0 && <StepWelcome />}
        {step === 1 && <StepUnified />}
        {step === 2 && <StepTrust />}

        <button
          onClick={handleNext}
          className="mt-10 flex items-center gap-2 rounded-xl bg-primary px-7 py-2.5 text-[14px] font-medium text-primary-foreground shadow-lg shadow-primary/25 transition-all hover:shadow-xl hover:shadow-primary/30 hover:scale-[1.02] active:scale-[0.98]"
        >
          {isLast ? "Get Started" : "Next"}
          <ArrowRight size={15} className="opacity-60" />
        </button>
      </div>
    </div>
  );
}

/* ══════════════════════════════════════════════════════
   Step 0: Welcome (unchanged)
   ══════════════════════════════════════════════════════ */

const AGENTS = [
  "claude",
  "codex",
  "gemini",
  "cursor",
  "antigravity",
  "copilot",
  "windsurf",
] as const;
const FLOAT_DELAYS = [0, 0.4, 0.9, 1.3, 0.6, 1.1, 1.6];
const SCATTER_POSITIONS = [
  { x: -140, y: -80, r: -15 },
  { x: 100, y: -90, r: 12 },
  { x: 160, y: 50, r: -8 },
  { x: -120, y: 70, r: 10 },
  { x: -180, y: 10, r: -20 },
  { x: 150, y: 80, r: 15 },
  { x: 0, y: 108, r: -6 },
];

function HandAnnotation({
  type,
  delay = 2400,
  children,
}: {
  type: "circle" | "underline" | "highlight";
  delay?: number;
  children: React.ReactNode;
}) {
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!ref.current) return;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- roughness is supported at runtime but missing from type defs
    const a = annotate(ref.current, {
      type,
      color: "var(--primary)",
      strokeWidth: 1.5,
      padding: type === "circle" ? [2, 6, 4, 10] : 2,
      iterations: 1,
      animationDuration: type === "circle" ? 600 : 400,
      roughness: type === "circle" ? 0.6 : 0.8,
      ...(type === "highlight" && {
        color: "color-mix(in oklch, var(--primary) 15%, transparent)",
      }),
    } as any);
    const timer = setTimeout(() => a.show(), delay);
    return () => {
      clearTimeout(timer);
      a.remove();
    };
  }, [type, delay]);

  return <span ref={ref}>{children}</span>;
}

function StepWelcome() {
  const [gathered, setGathered] = useState(false);
  const [floating, setFloating] = useState(false);

  useEffect(() => {
    const t1 = setTimeout(() => setGathered(true), 500);
    const t2 = setTimeout(() => setFloating(true), 2600);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, []);

  return (
    <div className="flex flex-col items-center text-center">
      <div className="relative mb-12">
        <div className="relative flex items-center justify-center gap-4">
          {AGENTS.map((name, i) => {
            const s = SCATTER_POSITIONS[i];
            return (
              <div
                key={name}
                style={{
                  transform: gathered
                    ? "translate(0, 0) rotate(0deg)"
                    : `translate(${s.x}px, ${s.y}px) rotate(${s.r}deg)`,
                  opacity: gathered ? 1 : 0,
                  transition: `transform 1400ms cubic-bezier(0.34, 1.56, 0.64, 1) ${i * 100}ms, opacity 800ms ease-out ${i * 100}ms`,
                  animation: floating
                    ? `onboarding-float 3s ease-in-out ${FLOAT_DELAYS[i]}s infinite`
                    : "none",
                }}
              >
                <AgentMascot name={name} size={52} />
              </div>
            );
          })}
        </div>
      </div>

      <h1 className="font-serif text-[44px] font-semibold tracking-tight leading-[1.1] text-foreground">
        Welcome to <span style={shimmerStyle}>HarnessKit</span>
      </h1>
      <p className="mt-3 text-[15px] font-medium text-primary/70">
        One home for every agent
      </p>
      <p className="mt-7 max-w-[500px] text-[15px] leading-[1.85] text-muted-foreground">
        Every agent, a different world.
        <br />
        <HandAnnotation type="highlight" delay={2000}>
          Extensions, configs, memory, and rules
        </HandAnnotation>{" "}
        —{" "}
        <HandAnnotation type="circle" delay={2600}>
          scattered everywhere
        </HandAnnotation>
        .
        <br />
        HarnessKit brings them all{" "}
        <span style={shimmerStyle}>under one roof</span>.
      </p>
    </div>
  );
}

/* ══════════════════════════════════════════════════════
   Step 1: Everything, unified (split layout)
   ══════════════════════════════════════════════════════ */

function StepUnified() {
  return (
    <div className="flex w-full flex-col items-center">
      <h1 className="text-center font-serif text-[40px] font-semibold tracking-tight leading-[1.1] text-foreground">
        All your agents, <span style={shimmerStyle}>one view</span>
      </h1>

      <p className="mt-3 mb-6 text-center text-[14px] text-muted-foreground/70">
        Extensions, configs, memory, and rules — managed together, synced across
        agents.
      </p>

      {/* Two mocks side by side — same height as page 3 */}
      <div className="flex w-full gap-4" style={{ height: 300 }}>
        <div style={{ width: "calc(50% - 8px)", height: 300 }}>
          <MockExtensionsPreview />
        </div>
        <div style={{ width: "calc(50% - 8px)", height: 300 }}>
          <MockWindowCard delay={200}>
            <MockAgentFilesPreview />
          </MockWindowCard>
        </div>
      </div>

      {/* Core highlights */}
      <div className="mt-5 flex flex-wrap justify-center gap-2">
        {[
          "One click to deploy across agents",
          "Format differences handled automatically",
          "Config files auto-discovered in real time",
          "And more...",
        ].map((text, i) => (
          <div
            key={text}
            className="flex items-center gap-2 rounded-lg px-3 py-1.5"
            style={{
              animation: `fade-in 400ms ease-out ${(i + 1) * 100 + 800}ms both`,
              background:
                i < 3
                  ? "color-mix(in oklch, var(--primary) 4%, var(--card))"
                  : "transparent",
              border:
                i < 3
                  ? "1px solid color-mix(in oklch, var(--primary) 10%, transparent)"
                  : "1px solid transparent",
            }}
          >
            {i < 3 && (
              <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-primary/40" />
            )}
            <span
              className={`text-[12px] ${i < 3 ? "text-foreground/65" : "text-muted-foreground/40 italic"}`}
            >
              {text}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

/* ══════════════════════════════════════════════════════
   Step 2: Trust, built in (split layout)
   ══════════════════════════════════════════════════════ */

function StepTrust() {
  return (
    <div className="flex w-full flex-col items-center">
      <h1 className="text-center font-serif text-[40px] font-semibold tracking-tight leading-[1.1] text-foreground">
        Trust, <span style={shimmerStyle}>built in</span>
      </h1>

      <p className="mt-3 mb-6 text-center text-[14px] text-muted-foreground/70">
        Every extension is audited with a trust score — pinpointed to the exact
        file and line.
      </p>

      {/* Audit + Marketplace side by side, equal */}
      <div className="flex w-full gap-4" style={{ height: 300 }}>
        <div style={{ width: "calc(50% - 8px)", height: 300 }}>
          <MockAuditPreview />
        </div>
        <div style={{ width: "calc(50% - 8px)", height: 300 }}>
          <MockMarketplacePreview />
        </div>
      </div>

      {/* Core highlights */}
      <div className="mt-5 flex flex-wrap justify-center gap-2">
        {[
          "18 static analysis rules",
          "Per-agent independent scanning",
          "Audit and source visibility before install",
          "And more...",
        ].map((text, i) => (
          <div
            key={text}
            className="flex items-center gap-2 rounded-lg px-3 py-1.5"
            style={{
              animation: `fade-in 400ms ease-out ${(i + 1) * 100 + 800}ms both`,
              background:
                i < 3
                  ? "color-mix(in oklch, var(--primary) 4%, var(--card))"
                  : "transparent",
              border:
                i < 3
                  ? "1px solid color-mix(in oklch, var(--primary) 10%, transparent)"
                  : "1px solid transparent",
            }}
          >
            {i < 3 && (
              <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-primary/40" />
            )}
            <span
              className={`text-[12px] ${i < 3 ? "text-foreground/65" : "text-muted-foreground/40 italic"}`}
            >
              {text}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

/* ══════════════════════════════════════════════════════
   Shared: MockWindowCard
   ══════════════════════════════════════════════════════ */

function MockWindowCard({
  children,
  delay = 0,
}: {
  children: React.ReactNode;
  delay?: number;
}) {
  return (
    <div
      className="relative rounded-2xl overflow-hidden px-3.5 py-2.5 h-full flex flex-col"
      style={{
        animation: `entrance-spring 600ms cubic-bezier(0.34, 1.56, 0.64, 1) ${delay}ms both`,
        background: "color-mix(in oklch, var(--card) 80%, transparent)",
        border: "1px solid color-mix(in oklch, var(--primary) 8%, transparent)",
        boxShadow:
          "0 8px 32px -4px color-mix(in oklch, var(--primary) 10%, transparent), 0 2px 4px color-mix(in oklch, var(--primary) 4%, transparent)",
        backdropFilter: "blur(16px)",
        WebkitBackdropFilter: "blur(16px)",
      }}
    >
      <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden">
        {children}
      </div>
    </div>
  );
}

/* ══════════════════════════════════════════════════════
   Shared: MockCursor
   ══════════════════════════════════════════════════════ */

function MockCursor({
  x,
  y,
  visible,
  clicking,
}: {
  x: number;
  y: number;
  visible: boolean;
  clicking: boolean;
}) {
  return (
    <div
      className="pointer-events-none absolute z-50 transition-all duration-500"
      style={{
        left: x,
        top: y,
        opacity: visible ? 1 : 0,
        transitionTimingFunction: "cubic-bezier(0.22, 1, 0.36, 1)",
        transform: clicking ? "scale(0.75)" : "scale(1)",
        transitionProperty: "left, top, opacity, transform",
        transitionDuration: visible ? "500ms, 500ms, 200ms, 100ms" : "0ms",
      }}
    >
      <svg width="14" height="18" viewBox="0 0 14 18" fill="none">
        <path
          d="M1 1L13 9.5L7.5 10.5L10 17L7.5 16L5 10.5L1 13.5V1Z"
          fill="var(--foreground)"
          fillOpacity="0.6"
          stroke="var(--background)"
          strokeWidth="0.5"
        />
      </svg>
    </div>
  );
}

/* ══════════════════════════════════════════════════════
   Mock Extensions Preview (Page 2)
   ══════════════════════════════════════════════════════ */

const FILTER_KINDS = ["All", "skill", "mcp", "plugin", "hook", "cli"];

function MockExtensionsPreview() {
  const [toggledRow, setToggledRow] = useState(-1);
  const [cursor, setCursor] = useState({
    x: 0,
    y: 0,
    visible: false,
    clicking: false,
  });

  useEffect(() => {
    let active = true;
    let timers: ReturnType<typeof setTimeout>[] = [];
    const t = (ms: number, fn: () => void) => {
      timers.push(
        setTimeout(() => {
          if (active) fn();
        }, ms),
      );
    };
    const runCycle = () => {
      timers.forEach(clearTimeout);
      timers = [];
      setToggledRow(-1);
      setCursor({ x: 0, y: 0, visible: false, clicking: false });
      t(1400, () =>
        setCursor({ x: 480, y: 95, visible: true, clicking: false }),
      );
      t(1800, () => setCursor((c) => ({ ...c, clicking: true })));
      t(1900, () => {
        setCursor((c) => ({ ...c, clicking: false }));
        setToggledRow(0);
      });
      t(2800, () => setCursor((c) => ({ ...c, clicking: true })));
      t(2900, () => {
        setCursor((c) => ({ ...c, clicking: false }));
        setToggledRow(-1);
      });
      t(3600, () => setCursor((c) => ({ ...c, visible: false })));
      t(8000, runCycle);
    };
    runCycle();
    return () => {
      active = false;
      timers.forEach(clearTimeout);
    };
  }, []);

  return (
    <MockWindowCard>
      <div
        className="mb-1.5"
        style={{ animation: "fade-in 300ms ease-out 50ms both" }}
      >
        <span className="text-[12px] font-bold text-foreground/70">
          Extensions
        </span>
      </div>
      <div className="flex gap-1.5 mb-2">
        {FILTER_KINDS.map((kind, i) => (
          <span
            key={kind}
            className="rounded-md px-2 py-0.5 text-[9px] font-medium"
            style={{
              animation: `scale-in 300ms ease-out ${i * 40 + 100}ms both`,
              background:
                kind === "All"
                  ? "color-mix(in oklch, var(--primary) 15%, transparent)"
                  : "var(--muted)",
              color:
                kind === "All" ? "var(--primary)" : "var(--muted-foreground)",
            }}
          >
            {kind}
          </span>
        ))}
      </div>
      <table
        className="w-full"
        style={{ borderSpacing: "0 2px", borderCollapse: "separate" }}
      >
        <thead>
          <tr className="text-[8px] font-semibold uppercase tracking-wider text-muted-foreground/40">
            <td className="pb-1 pl-2" style={{ width: "20%" }}>
              Name
            </td>
            <td className="pb-1 text-center">Kind</td>
            <td className="pb-1 text-center">Agent</td>
            <td className="pb-1 text-center">Perms</td>
            <td className="pb-1 text-center">Score</td>
            <td className="pb-1 text-center pr-2">Status</td>
          </tr>
        </thead>
        <tbody>
          {MOCK_EXTENSIONS.map((ext, i) => (
            <MockExtRow
              key={ext.kind}
              ext={ext}
              index={i}
              toggled={i === toggledRow}
            />
          ))}
        </tbody>
      </table>
      <MockCursor {...cursor} />
    </MockWindowCard>
  );
}

function MockExtRow({
  ext,
  index,
  toggled,
}: {
  ext: (typeof MOCK_EXTENSIONS)[number];
  index: number;
  toggled: boolean;
}) {
  const score = ext.score;
  const isEnabled = toggled ? !ext.enabled : ext.enabled;
  const tierColor =
    score >= 80
      ? "var(--trust-safe)"
      : score >= 60
        ? "var(--trust-low-risk)"
        : "var(--trust-high-risk)";

  return (
    <tr
      className="rounded-lg"
      style={{ animation: `fade-in 400ms ease-out ${200 + index * 80}ms both` }}
    >
      <td className="py-1.5 pl-2">
        <div
          className="h-2 rounded-full bg-foreground/10"
          style={{ width: [48, 40, 56, 44, 36][index] }}
        />
      </td>
      <td className="py-1.5 text-center">
        <span
          className="inline-block rounded px-2 py-0.5 text-[10px] font-semibold"
          style={{
            color: KIND_COLORS[ext.kind],
            background: `color-mix(in oklch, ${KIND_COLORS[ext.kind]} 12%, transparent)`,
          }}
        >
          {ext.kind}
        </span>
      </td>
      <td className="py-1.5">
        <div className="flex justify-center gap-1">
          {ext.agents.map((a) => (
            <AgentMascot key={a} name={a} size={14} />
          ))}
        </div>
      </td>
      <td className="py-1.5">
        <div className="flex justify-center gap-0.5">
          {ext.perms.map((p) => {
            const Icon = PERM_ICONS[p];
            return (
              <Icon key={p} size={11} className="text-muted-foreground/40" />
            );
          })}
        </div>
      </td>
      <td
        className="py-1.5 text-center text-[11px] font-mono font-semibold"
        style={{ color: tierColor }}
      >
        {score}
      </td>
      <td className="py-1.5 text-center pr-2">
        <span
          className="inline-block rounded-md py-0.5 text-[9px] font-medium text-center transition-all duration-300"
          style={{
            width: 50,
            background: isEnabled
              ? "color-mix(in oklch, var(--primary) 15%, transparent)"
              : "var(--muted)",
            color: isEnabled ? "var(--primary)" : "var(--muted-foreground)",
          }}
        >
          {isEnabled ? "enabled" : "disabled"}
        </span>
      </td>
    </tr>
  );
}

/* ── Mock Agent Files Preview (sidebar + detail) ── */

const MOCK_AGENT_SIDEBAR = [
  { id: "claude", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini CLI" },
  { id: "cursor", label: "Cursor" },
  { id: "antigravity", label: "Antigravity" },
  { id: "copilot", label: "Copilot" },
  { id: "windsurf", label: "Windsurf" },
] as const;

const MOCK_FILES = [
  {
    id: "rules-g",
    label: "Rules",
    name: "CLAUDE.md",
    scope: "Global" as const,
    path: "~/.claude/",
    size: "2.4 KB",
  },
  {
    id: "rules-p",
    label: "Rules",
    name: "CLAUDE.md",
    scope: "Project" as const,
    path: "~/projects/my-app/.claude/CLAUDE.md",
    size: "1.1 KB",
  },
  {
    id: "memory",
    label: "Memory",
    name: "MEMORY.md",
    scope: "Global" as const,
    path: "~/.claude/projects/<project>/memory/",
    size: "1.8 KB",
  },
  {
    id: "settings-g",
    label: "Settings",
    name: "settings.json",
    scope: "Global" as const,
    path: "~/.claude/",
    size: "856 B",
  },
];

const MOCK_FILE_PREVIEW = `# Project Rules

- Follow existing code patterns
- Run tests before committing
- Write clear commit messages`;

function MockAgentFilesPreview() {
  const [expandedFile, setExpandedFile] = useState("");
  const [cursor, setCursor] = useState({
    x: 0,
    y: 0,
    visible: false,
    clicking: false,
  });

  useEffect(() => {
    let active = true;
    let timers: ReturnType<typeof setTimeout>[] = [];
    const t = (ms: number, fn: () => void) => {
      timers.push(
        setTimeout(() => {
          if (active) fn();
        }, ms),
      );
    };
    const runCycle = () => {
      timers.forEach(clearTimeout);
      timers = [];
      setExpandedFile("");
      setCursor({ x: 0, y: 0, visible: false, clicking: false });
      t(4200, () =>
        setCursor({ x: 140, y: 48, visible: true, clicking: false }),
      );
      t(4600, () => setCursor((c) => ({ ...c, clicking: true })));
      t(4700, () => {
        setCursor((c) => ({ ...c, clicking: false }));
        setExpandedFile("rules-g");
      });
      t(5600, () => setCursor((c) => ({ ...c, visible: false })));
      t(8000, runCycle);
    };
    runCycle();
    return () => {
      active = false;
      timers.forEach(clearTimeout);
    };
  }, []);

  // Group files by label
  const groups = MOCK_FILES.reduce<Record<string, typeof MOCK_FILES>>(
    (acc, f) => {
      if (!acc[f.label]) acc[f.label] = [];
      acc[f.label].push(f);
      return acc;
    },
    {},
  );

  return (
    <div
      className="relative flex flex-col h-full"
      style={{ animation: "fade-in 400ms ease-out 400ms both" }}
    >
      <div
        className="mb-1.5 shrink-0"
        style={{ animation: "fade-in 300ms ease-out 250ms both" }}
      >
        <span className="text-[12px] font-bold text-foreground/70">Agents</span>
      </div>
      <div className="flex gap-0 flex-1 min-h-0">
        {/* Sidebar — all 6 agents with names */}
        <div
          className="flex flex-col gap-0.5 border-r pr-2 mr-3 shrink-0"
          style={{
            borderColor: "color-mix(in oklch, var(--border) 50%, transparent)",
          }}
        >
          {MOCK_AGENT_SIDEBAR.map(({ id, label }, i) => (
            <div
              key={id}
              className="flex items-center gap-1.5 rounded-md px-1.5 py-1"
              style={{
                animation: `fade-in 300ms ease-out ${500 + i * 50}ms both`,
                background:
                  i === 0
                    ? "color-mix(in oklch, var(--primary) 8%, transparent)"
                    : "transparent",
              }}
            >
              <AgentMascot name={id} size={14} />
              <span className="text-[9px] text-muted-foreground/50 whitespace-nowrap">
                {label}
              </span>
            </div>
          ))}
        </div>

        {/* Detail area — scrollable */}
        <div className="flex-1 flex flex-col gap-1.5 min-w-0 overflow-y-auto">
          {/* File categories */}
          {Object.entries(groups).map(([label, files], ci) => (
            <div
              key={label}
              style={{
                animation: `fade-in 300ms ease-out ${650 + ci * 60}ms both`,
              }}
            >
              <div className="text-[8px] font-semibold uppercase tracking-wider text-muted-foreground/40 mb-0.5">
                {label}
              </div>
              {files.map((f) => (
                <div key={f.id}>
                  <div className="flex items-center gap-2 py-0.5 ml-1 rounded px-1">
                    <ChevronRight
                      size={8}
                      className="text-muted-foreground/30 shrink-0"
                      style={{
                        transform:
                          expandedFile === f.id ? "rotate(90deg)" : "rotate(0)",
                        transition: "transform 200ms",
                      }}
                    />
                    <span className="text-[10px] font-mono text-foreground/60">
                      {f.name}
                    </span>
                    <span
                      className="rounded px-1 py-px text-[7px] font-medium shrink-0"
                      style={{
                        background:
                          f.scope === "Global"
                            ? "color-mix(in oklch, var(--primary) 8%, transparent)"
                            : "oklch(0.55 0.08 175 / 0.08)",
                        color:
                          f.scope === "Global"
                            ? "var(--primary)"
                            : "oklch(0.48 0.08 175)",
                      }}
                    >
                      {f.scope}
                    </span>
                    <span className="text-[8px] font-mono text-muted-foreground/25 truncate ml-auto">
                      {f.path}
                    </span>
                    <span className="text-[8px] text-muted-foreground/30 shrink-0">
                      {f.size}
                    </span>
                  </div>
                  {/* Expanded file preview — matches real app */}
                  {expandedFile === f.id && (
                    <div
                      className="ml-1 mr-1 mt-1 mb-1.5 border-t overflow-hidden"
                      style={{
                        animation: "mock-expand 300ms ease-out both",
                        borderColor:
                          "color-mix(in oklch, var(--border) 30%, transparent)",
                        background:
                          "color-mix(in oklch, var(--muted) 30%, transparent)",
                      }}
                    >
                      <div className="px-3 py-2">
                        {/* Code preview */}
                        <pre className="text-[8px] leading-[1.6] font-mono text-muted-foreground/70 whitespace-pre-wrap mb-2 max-h-[60px] overflow-hidden">
                          {MOCK_FILE_PREVIEW}
                        </pre>
                        {/* Action buttons */}
                        <div className="flex gap-1.5">
                          <span
                            className="inline-flex items-center rounded-md border px-2 py-0.5 text-[7px] font-medium text-foreground/50"
                            style={{
                              borderColor:
                                "color-mix(in oklch, var(--border) 60%, transparent)",
                            }}
                          >
                            Open in Editor
                          </span>
                          <span
                            className="inline-flex items-center rounded-md border px-2 py-0.5 text-[7px] font-medium text-foreground/50"
                            style={{
                              borderColor:
                                "color-mix(in oklch, var(--border) 60%, transparent)",
                            }}
                          >
                            Reveal in Finder
                          </span>
                          <span
                            className="inline-flex items-center rounded-md border px-2 py-0.5 text-[7px] font-medium text-foreground/50"
                            style={{
                              borderColor:
                                "color-mix(in oklch, var(--border) 60%, transparent)",
                            }}
                          >
                            Copy Path
                          </span>
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
      <MockCursor {...cursor} />
    </div>
  );
}

/* ══════════════════════════════════════════════════════
   Mock Audit Preview (Page 3)
   ══════════════════════════════════════════════════════ */

function MockAuditPreview() {
  const [expanded, setExpanded] = useState(-1);
  const [cursor, setCursor] = useState({
    x: 0,
    y: 0,
    visible: false,
    clicking: false,
  });

  useEffect(() => {
    let active = true;
    let timers: ReturnType<typeof setTimeout>[] = [];
    const t = (ms: number, fn: () => void) => {
      timers.push(
        setTimeout(() => {
          if (active) fn();
        }, ms),
      );
    };
    const runCycle = () => {
      timers.forEach(clearTimeout);
      timers = [];
      setExpanded(-1);
      setCursor({ x: 0, y: 0, visible: false, clicking: false });
      t(1600, () =>
        setCursor({ x: 200, y: 84, visible: true, clicking: false }),
      );
      t(2200, () => setCursor((c) => ({ ...c, clicking: true })));
      t(2400, () => {
        setCursor((c) => ({ ...c, clicking: false }));
        setExpanded(0);
      });
      t(3400, () => setCursor((c) => ({ ...c, visible: false })));
      t(8000, runCycle);
    };
    runCycle();
    return () => {
      active = false;
      timers.forEach(clearTimeout);
    };
  }, []);

  return (
    <MockWindowCard>
      {/* Title */}
      <div
        className="mb-1.5"
        style={{ animation: "fade-in 300ms ease-out 150ms both" }}
      >
        <span className="text-[12px] font-bold text-foreground/70">
          Security Audit
        </span>
      </div>

      {/* Header */}
      <div className="flex items-center gap-3 mb-1.5">
        <span className="rounded-md bg-primary/10 px-2.5 py-1 text-[10px] font-medium text-primary/70">
          Run Audit
        </span>
        <span className="text-[10px] text-muted-foreground/50">
          4 extensions scanned · Last run 2 min ago
        </span>
      </div>

      {/* Audit results */}
      <div className="flex flex-col gap-0.5">
        {MOCK_AUDIT.map((item, i) => (
          <MockAuditRow
            key={item.name}
            item={item}
            index={i}
            isExpanded={i === expanded}
          />
        ))}
      </div>

      <MockCursor {...cursor} />
    </MockWindowCard>
  );
}

const SEVERITY_COLORS: Record<string, string> = {
  High: "var(--trust-high-risk)",
  Medium: "var(--trust-low-risk)",
  Low: "var(--muted-foreground)",
};

function MockAuditRow({
  item,
  index,
  isExpanded,
}: {
  item: (typeof MOCK_AUDIT)[number];
  index: number;
  isExpanded: boolean;
}) {
  const tierColor =
    item.score >= 80
      ? "var(--trust-safe)"
      : item.score >= 60
        ? "var(--trust-low-risk)"
        : "var(--trust-high-risk)";

  if (item.clean) {
    return (
      <div
        className="flex items-center gap-2.5 rounded-lg px-3 py-2"
        style={{
          animation: `fade-in 400ms ease-out ${200 + index * 80}ms both`,
        }}
      >
        <Check size={13} style={{ color: "var(--trust-safe)" }} />
        <span className="flex-1 text-[11px] text-muted-foreground/60">
          {item.name}
        </span>
        <span className="text-[9px] text-muted-foreground/35">Clean</span>
      </div>
    );
  }

  return (
    <div
      className="rounded-xl overflow-hidden"
      style={{
        animation: `fade-in 400ms ease-out ${200 + index * 80}ms both`,
        border: isExpanded
          ? "1px solid color-mix(in oklch, var(--border) 60%, transparent)"
          : "1px solid transparent",
        background: isExpanded
          ? "color-mix(in oklch, var(--card) 50%, transparent)"
          : "transparent",
        transition: "border-color 200ms, background 200ms",
      }}
    >
      {/* Collapsed row */}
      <div className="flex items-center gap-2.5 px-3 py-2">
        <ChevronRight
          size={13}
          className="text-muted-foreground/50 shrink-0"
          style={{
            transform: isExpanded ? "rotate(90deg)" : "rotate(0)",
            transition: "transform 200ms",
          }}
        />
        <span className="flex-1 text-[11px] font-medium text-foreground/70">
          {item.name}
        </span>
        {"findings" in item && (
          <span className="text-[9px] text-muted-foreground/40">
            {item.findings} finding{item.findings !== 1 ? "s" : ""}
          </span>
        )}
        <span
          className="text-[11px] font-mono font-semibold"
          style={{ color: tierColor }}
        >
          {item.score}
        </span>
      </div>

      {/* Expanded details — rules + findings with file:line */}
      {isExpanded && "rules" in item && (
        <div
          className="px-3 pb-3"
          style={{ animation: "fade-in 200ms ease-out both" }}
        >
          <div
            className="border-t pt-2 flex flex-col gap-1.5"
            style={{
              borderColor:
                "color-mix(in oklch, var(--border) 40%, transparent)",
            }}
          >
            {(
              item as {
                rules: {
                  name: string;
                  severity: string;
                  findings: { loc: string }[];
                }[];
              }
            ).rules.map((rule) => {
              const sevColor =
                SEVERITY_COLORS[rule.severity] || "var(--muted-foreground)";
              return (
                <div key={rule.name} className="flex flex-col gap-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[10px] font-medium text-foreground/65">
                      {rule.name}
                    </span>
                    <span
                      className="rounded px-1.5 py-0.5 text-[8px] font-semibold"
                      style={{
                        color: sevColor,
                        background: `color-mix(in oklch, ${sevColor} 12%, transparent)`,
                      }}
                    >
                      {rule.severity}
                    </span>
                  </div>
                  {rule.findings.map((f) => (
                    <div
                      key={f.loc}
                      className="ml-2 rounded px-2 py-0.5"
                      style={{
                        background:
                          "color-mix(in oklch, var(--muted) 40%, transparent)",
                      }}
                    >
                      <span className="text-[9px] font-mono text-muted-foreground/50">
                        {f.loc}
                      </span>
                    </div>
                  ))}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

/* ══════════════════════════════════════════════════════
   Mock Marketplace Preview (Page 3, smaller)
   ══════════════════════════════════════════════════════ */

const MOCK_MARKETPLACE_LIST = [
  { name: "my-skill", installs: "2.1K", desc: true },
  { name: "my-tool", installs: "1.8K", desc: true },
  { name: "my-helper", installs: "956", desc: true },
];

const MOCK_AUDITORS = [
  { name: "Anthropic Trust Hub", risk: "Safe" as const },
  { name: "Socket", risk: "Safe" as const },
  { name: "Snyk", risk: "Low" as const },
];

function MockMarketplacePreview() {
  const [selectedRow, setSelectedRow] = useState(-1);
  const [cursor, setCursor] = useState({
    x: 0,
    y: 0,
    visible: false,
    clicking: false,
  });

  useEffect(() => {
    let active = true;
    let timers: ReturnType<typeof setTimeout>[] = [];
    const t = (ms: number, fn: () => void) => {
      timers.push(
        setTimeout(() => {
          if (active) fn();
        }, ms),
      );
    };
    const runCycle = () => {
      timers.forEach(clearTimeout);
      timers = [];
      setSelectedRow(-1);
      setCursor({ x: 0, y: 0, visible: false, clicking: false });
      t(4000, () =>
        setCursor({ x: 60, y: 95, visible: true, clicking: false }),
      );
      t(4500, () => setCursor((c) => ({ ...c, clicking: true })));
      t(4600, () => {
        setCursor((c) => ({ ...c, clicking: false }));
        setSelectedRow(0);
      });
      t(5800, () => setCursor((c) => ({ ...c, visible: false })));
      t(8000, runCycle);
    };
    runCycle();
    return () => {
      active = false;
      timers.forEach(clearTimeout);
    };
  }, []);

  return (
    <MockWindowCard delay={200}>
      {/* Title */}
      <div
        className="mb-1.5"
        style={{ animation: "fade-in 300ms ease-out 150ms both" }}
      >
        <span className="text-[12px] font-bold text-foreground/70">
          Marketplace
        </span>
      </div>

      {/* Header: Install buttons + Search */}
      <div className="flex items-center gap-1.5 mb-2">
        {["Install from Git", "Install from Local"].map((label, i) => (
          <span
            key={label}
            className="rounded-md bg-primary/10 px-2.5 py-1 text-[10px] font-medium text-primary/70 shrink-0"
            style={{
              animation: `scale-in 300ms ease-out ${i * 40 + 200}ms both`,
            }}
          >
            {label}
          </span>
        ))}
        <span
          className="ml-auto rounded-md border border-border/60 px-2.5 py-1 text-[10px] text-muted-foreground/40 shrink-0"
          style={{ animation: "fade-in 300ms ease-out 300ms both", width: 100 }}
        >
          Search...
        </span>
      </div>

      {/* List + Detail */}
      <div className="flex h-full min-h-0 relative">
        {/* Skill list */}
        <div
          className="flex flex-col gap-0.5 overflow-y-auto"
          style={{
            width: selectedRow >= 0 ? "45%" : "100%",
            transition: "width 300ms ease",
          }}
        >
          {MOCK_MARKETPLACE_LIST.map((item, i) => (
            <div
              key={i}
              className="flex items-start gap-2 rounded-lg px-2 py-1.5"
              style={{
                animation: `fade-in 400ms ease-out ${400 + i * 70}ms both`,
                background:
                  i === selectedRow
                    ? "color-mix(in oklch, var(--primary) 6%, transparent)"
                    : "transparent",
                border:
                  i === selectedRow
                    ? "1px solid color-mix(in oklch, var(--primary) 12%, transparent)"
                    : "1px solid transparent",
              }}
            >
              <div className="flex-1 min-w-0">
                <div className="text-[10px] font-medium text-foreground/70">
                  {item.name}
                </div>
                <div
                  className="mt-0.5 h-1.5 rounded-full bg-foreground/5"
                  style={{ width: "90%" }}
                />
                <span className="text-[7px] text-muted-foreground/30">
                  {item.installs} installs
                </span>
              </div>
            </div>
          ))}
        </div>

        {/* Detail panel — slides in from right */}
        {selectedRow >= 0 && (
          <div
            className="absolute right-0 top-0 bottom-0 flex flex-col gap-2 overflow-y-auto border-l pl-2.5 bg-card/90"
            style={{
              width: "55%",
              borderColor:
                "color-mix(in oklch, var(--border) 40%, transparent)",
              animation: "slide-in-right 250ms ease-out both",
              backdropFilter: "blur(8px)",
            }}
          >
            {/* Header */}
            <div className="flex items-center gap-2">
              <span className="text-[11px] font-semibold text-foreground/80">
                my-skill
              </span>
              <span className="text-[8px] text-muted-foreground/40">
                2.1K installs · owner/repo
              </span>
            </div>

            {/* Description */}
            <div className="flex flex-col gap-0.5">
              <div className="h-1.5 w-full rounded-full bg-foreground/5" />
              <div className="h-1.5 w-4/5 rounded-full bg-foreground/5" />
            </div>

            {/* Security Audit — the key section */}
            <div
              className="rounded-lg p-2"
              style={{
                border:
                  "1px solid color-mix(in oklch, var(--border) 40%, transparent)",
              }}
            >
              <div className="text-[8px] font-semibold text-muted-foreground/50 mb-1.5">
                Security Audit
              </div>
              <div className="flex flex-col gap-1.5">
                {MOCK_AUDITORS.map((a) => (
                  <div
                    key={a.name}
                    className="flex items-center justify-between"
                  >
                    <span className="text-[9px] text-muted-foreground/60">
                      {a.name}
                    </span>
                    <span
                      className="rounded px-1.5 py-0.5 text-[7px] font-semibold"
                      style={{
                        color:
                          a.risk === "Safe"
                            ? "var(--trust-safe)"
                            : "var(--trust-low-risk)",
                        background:
                          a.risk === "Safe"
                            ? "color-mix(in oklch, var(--trust-safe) 10%, transparent)"
                            : "color-mix(in oklch, var(--trust-low-risk) 10%, transparent)",
                      }}
                    >
                      {a.risk}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            {/* Install to Agent */}
            <div className="text-[8px] font-semibold text-muted-foreground/50 mb-0.5">
              Install to Agent
            </div>
            <div className="flex flex-wrap gap-1">
              {(
                [
                  ["claude", "Claude Code"],
                  ["codex", "Codex"],
                  ["gemini", "Gemini CLI"],
                  ["cursor", "Cursor"],
                  ["antigravity", "Antigravity"],
                  ["copilot", "Copilot"],
                  ["windsurf", "Windsurf"],
                ] as const
              ).map(([id, label]) => (
                <div
                  key={id}
                  className="flex items-center gap-1 rounded-md border px-1.5 py-0.5"
                  style={{
                    borderColor:
                      "color-mix(in oklch, var(--border) 50%, transparent)",
                  }}
                >
                  <AgentMascot name={id} size={10} />
                  <span className="text-[7px] text-foreground/50">{label}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      <MockCursor {...cursor} />
    </MockWindowCard>
  );
}
