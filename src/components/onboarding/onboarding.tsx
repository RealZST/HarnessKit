import { useState, useEffect, useRef } from "react";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import { Shield, ShoppingBag, ArrowRight } from "lucide-react";

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

interface OnboardingProps {
  onComplete: () => void;
}

const STEPS = [
  {
    title: "Welcome to HarnessKit",
    subtitle: "Your AI agent extension manager",
    body: "Manage skills, MCP servers, hooks, and plugins across all your coding agents — from one place.",
  },
  {
    title: "Monitor & Secure",
    subtitle: "Keep your extensions safe",
    body: "Audit extensions for security risks, track permissions, and get trust scores to know what's running in your environment.",
    icon: Shield,
  },
  {
    title: "Discover & Install",
    subtitle: "Extend your agents",
    body: "Browse the marketplace or install from Git. Deploy skills to any agent with a single click.",
    icon: ShoppingBag,
  },
];

export function Onboarding({ onComplete }: OnboardingProps) {
  const [step, setStep] = useState(0);
  const [visible, setVisible] = useState(false);
  const [exiting, setExiting] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  // Fade in on mount
  useEffect(() => {
    requestAnimationFrame(() => setVisible(true));
  }, []);

  // Focus trap: focus the container when visible, and cycle Tab within the modal
  useEffect(() => {
    if (!visible || !containerRef.current) return;
    containerRef.current.focus();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Tab" || !containerRef.current) return;

      const focusableSelectors = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';
      const focusableElements = Array.from(
        containerRef.current.querySelectorAll<HTMLElement>(focusableSelectors),
      ).filter((el) => !el.hasAttribute("disabled"));

      if (focusableElements.length === 0) return;

      const first = focusableElements[0];
      const last = focusableElements[focusableElements.length - 1];

      if (e.shiftKey) {
        if (document.activeElement === first || document.activeElement === containerRef.current) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [visible, step]);

  const floatDelays = [0, 0.4, 0.9, 1.3, 0.6, 1.1];

  const isLast = step === STEPS.length - 1;
  const current = STEPS[step];

  const handleNext = () => {
    if (isLast) {
      setExiting(true);
      setTimeout(onComplete, 350);
    } else {
      setStep((s) => s + 1);
    }
  };

  const handleSkip = () => {
    setExiting(true);
    setTimeout(onComplete, 350);
  };

  return (
    <div
      ref={containerRef}
      tabIndex={-1}
      className="fixed inset-0 z-[9999] flex items-center justify-center bg-background/80 backdrop-blur-md transition-opacity duration-300"
      style={{ opacity: visible && !exiting ? 1 : 0 }}
    >
      <div
        className="relative mx-4 w-full max-w-lg rounded-2xl border border-border bg-card p-8 shadow-xl transition-all duration-300"
        style={{
          opacity: visible && !exiting ? 1 : 0,
          transform: visible && !exiting ? "translateY(0) scale(1)" : "translateY(12px) scale(0.98)",
        }}
      >
        {/* Progress dots */}
        <div className="flex justify-center gap-1.5 mb-8">
          {STEPS.map((_, i) => (
            <div
              key={i}
              className={`h-1.5 rounded-full transition-all duration-300 ${
                i === step
                  ? "w-6 bg-primary"
                  : i < step
                    ? "w-1.5 bg-primary/40"
                    : "w-1.5 bg-border"
              }`}
            />
          ))}
        </div>

        {/* Content — keyed by step to trigger transition */}
        <div
          key={step}
          className="animate-fade-in text-center"
        >
          {/* Step 0: mascot hero */}
          {step === 0 && (
            <div className="mb-6 flex justify-center gap-3">
              {["claude", "codex", "gemini", "cursor", "antigravity", "copilot"].map((name, i) => (
                <div
                  key={name}
                  className="animate-fade-in"
                  style={{
                    animationDelay: `${i * 80}ms`,
                    animationFillMode: "both",
                    animation: `onboarding-float 3s ease-in-out ${floatDelays[i]}s infinite, fade-in 200ms ease-out ${i * 80}ms both`,
                  }}
                >
                  <AgentMascot name={name} size={40} />
                </div>
              ))}
            </div>
          )}

          {/* Step 1+: icon */}
          {step > 0 && current.icon && (
            <div className="mb-6 flex justify-center">
              <div className="flex size-14 items-center justify-center rounded-2xl bg-primary/10">
                <current.icon size={28} className="text-primary" />
              </div>
            </div>
          )}

          <h2 className="text-xl font-semibold tracking-tight">
            {current.title}
          </h2>
          <p className="mt-1 text-sm text-primary/80">
            {current.subtitle}
          </p>
          <p className="mt-4 text-sm leading-relaxed text-muted-foreground">
            {current.body}
          </p>
        </div>

        {/* Actions */}
        <div className="mt-8 flex items-center justify-between">
          <button
            onClick={handleSkip}
            className="text-xs text-muted-foreground/60 hover:text-muted-foreground transition-colors"
          >
            Skip
          </button>
          <button
            onClick={handleNext}
            className="flex items-center gap-2 rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground transition-all hover:bg-primary/90 hover:shadow-md active:scale-[0.98]"
          >
            {isLast ? "Get Started" : "Next"}
            <ArrowRight size={14} />
          </button>
        </div>
      </div>
    </div>
  );
}
