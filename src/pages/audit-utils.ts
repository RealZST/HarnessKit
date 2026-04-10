import type { AuditFinding, ExtensionKind, Severity } from "@/lib/types";

type Kind = ExtensionKind;

export const AUDIT_RULES = [
  {
    id: "prompt-injection",
    label: "Prompt Injection",
    severity: "Critical" as Severity,
    deduction: 25,
    description: "Extension content could manipulate the AI agent's behavior",
    kinds: ["skill", "plugin"] as Kind[],
  },
  {
    id: "rce",
    label: "Remote Code Execution",
    severity: "Critical" as Severity,
    deduction: 25,
    description: "Extension could execute arbitrary code on your machine",
    kinds: ["skill", "hook", "plugin"] as Kind[],
  },
  {
    id: "credential-theft",
    label: "Credential Theft",
    severity: "Critical" as Severity,
    deduction: 25,
    description: "Extension may attempt to access stored credentials",
    kinds: ["skill", "hook", "plugin"] as Kind[],
  },
  {
    id: "plaintext-secrets",
    label: "Plaintext Secrets",
    severity: "Critical" as Severity,
    deduction: 25,
    description: "API keys or tokens found in plain text",
    kinds: ["skill", "hook", "mcp", "plugin"] as Kind[],
  },
  {
    id: "safety-bypass",
    label: "Safety Bypass",
    severity: "Critical" as Severity,
    deduction: 25,
    description: "Extension attempts to disable agent safety features",
    kinds: ["skill", "hook"] as Kind[],
  },
  {
    id: "dangerous-commands",
    label: "Dangerous Commands",
    severity: "High" as Severity,
    deduction: 15,
    description: "Extension uses potentially harmful shell commands",
    kinds: ["skill", "hook", "plugin"] as Kind[],
  },
  {
    id: "broad-permissions",
    label: "Broad Permissions",
    severity: "High" as Severity,
    deduction: 15,
    description: "Extension requests more access than it needs",
    kinds: ["skill", "mcp", "hook"] as Kind[],
  },
  {
    id: "supply-chain",
    label: "Supply Chain Risk",
    severity: "Medium" as Severity,
    deduction: 8,
    description: "Dependencies may introduce security risks",
    kinds: ["skill", "mcp"] as Kind[],
  },
  {
    id: "unknown-source",
    label: "Unknown Source",
    severity: "Low" as Severity,
    deduction: 3,
    description: "Extension origin cannot be determined",
    kinds: ["skill", "mcp", "hook", "plugin"] as Kind[],
  },
  {
    id: "permission-combo-risk",
    label: "Permission Combination Risk",
    severity: "High" as Severity,
    deduction: 15,
    description:
      "Dangerous combination of permissions that could enable data exfiltration or RCE",
    kinds: ["skill", "mcp", "hook", "plugin"] as Kind[],
  },
  {
    id: "cli-credential-storage",
    label: "CLI Credential Storage",
    severity: "High" as Severity,
    deduction: 15,
    description:
      "CLI credential file has overly permissive permissions or unknown storage location",
    kinds: ["cli"] as Kind[],
  },
  {
    id: "cli-network-access",
    label: "CLI Network Access",
    severity: "Medium" as Severity,
    deduction: 8,
    description: "CLI connects to many external API domains",
    kinds: ["cli"] as Kind[],
  },
  {
    id: "cli-binary-source",
    label: "CLI Binary Source",
    severity: "High" as Severity,
    deduction: 15,
    description:
      "CLI binary was installed via untrusted method or has unknown origin",
    kinds: ["cli"] as Kind[],
  },
  {
    id: "cli-permission-scope",
    label: "CLI Permission Scope",
    severity: "Medium" as Severity,
    deduction: 8,
    description: "CLI child skills span many permission types",
    kinds: ["cli"] as Kind[],
  },
  {
    id: "cli-aggregate-risk",
    label: "CLI Aggregate Risk",
    severity: "Medium" as Severity,
    deduction: 8,
    description:
      "CLI child skills combine network, filesystem, and shell permissions",
    kinds: ["cli"] as Kind[],
  },
  {
    id: "mcp-command-injection",
    label: "MCP Command Injection",
    severity: "High" as Severity,
    deduction: 15,
    description:
      "MCP server args contain shell operators or injection patterns",
    kinds: ["mcp"] as Kind[],
  },
  {
    id: "plugin-source-trust",
    label: "Plugin Source Trust",
    severity: "Medium" as Severity,
    deduction: 8,
    description:
      "Plugin has no standard manifest file or no tracked Git origin",
    kinds: ["plugin"] as Kind[],
  },
  {
    id: "plugin-lifecycle-scripts",
    label: "Plugin Lifecycle Scripts",
    severity: "Medium" as Severity,
    deduction: 8,
    description:
      "Plugin contains lifecycle scripts (preinstall, postinstall) that run automatically",
    kinds: ["plugin"] as Kind[],
  },
] as const;

/** Return only the rules applicable to a given extension kind. */
export function rulesForKind(kind: ExtensionKind) {
  return AUDIT_RULES.filter((r) =>
    (r.kinds as readonly string[]).includes(kind),
  );
}

const SEVERITY_ORDER: Record<Severity, number> = {
  Critical: 3,
  High: 2,
  Medium: 1,
  Low: 0,
};

/** Return the highest severity among a list of findings. */
export function maxSeverity(findings: AuditFinding[]): Severity {
  let max: Severity = "Low";
  for (const f of findings) {
    if (SEVERITY_ORDER[f.severity] > SEVERITY_ORDER[max]) max = f.severity;
  }
  return max;
}

const SEVERITY_DEDUCTION: Record<Severity, number> = {
  Critical: 25,
  High: 15,
  Medium: 8,
  Low: 3,
};

/** Mirrors backend compute_trust_score: first hit per rule_id deducts full amount,
 *  subsequent hits of the same rule deduct 1 point each. */
export function computeTrustScore(findings: AuditFinding[]): number {
  const seen = new Set<string>();
  let deduction = 0;
  for (const f of findings) {
    if (seen.has(f.rule_id)) {
      deduction += 1;
    } else {
      seen.add(f.rule_id);
      deduction += SEVERITY_DEDUCTION[f.severity];
    }
  }
  return Math.max(0, 100 - deduction);
}

export function severityBadgeClass(severity: string): string {
  switch (severity) {
    case "Critical":
      return "bg-trust-critical/10 text-trust-critical";
    case "High":
      return "bg-trust-high-risk/10 text-trust-high-risk font-semibold";
    case "Medium":
      return "bg-trust-low-risk/10 text-trust-low-risk";
    case "Low":
      return "bg-muted text-muted-foreground";
    default:
      return "";
  }
}

export function severityIconColor(severity: string): string {
  switch (severity) {
    case "Critical":
      return "text-trust-critical";
    case "High":
      return "text-trust-high-risk";
    case "Medium":
      return "text-trust-low-risk";
    case "Low":
      return "text-muted-foreground";
    default:
      return "text-trust-critical";
  }
}

export interface GroupedResult {
  name: string;
  /** The primary extension kind for this group (used to filter applicable rules). */
  kind: ExtensionKind;
  /** Per-agent sub-results used for collecting findings across agents/kinds. */
  agents: {
    agent: string;
    id: string;
    findings: AuditFinding[];
    trust_score: number;
  }[];
  /** Trust score computed from all merged findings. */
  trust_score: number;
  /** Merged unique findings across all agents and kinds (deduped by rule_id for display). */
  findings: AuditFinding[];
  /** Primary ID for keying and scroll targets. */
  primaryId: string;
}
