import { describe, expect, it } from "vitest";
import type { AuditFinding } from "@/lib/types";
import {
  AUDIT_RULES,
  computeTrustScore,
  maxSeverity,
  severityBadgeClass,
  severityIconColor,
} from "../audit-utils";

/** Helper to build an AuditFinding with the given rule_id and severity. */
function finding(
  rule_id: string,
  severity: AuditFinding["severity"],
): AuditFinding {
  return { rule_id, severity, message: "test", location: "test" };
}

// ---------------------------------------------------------------------------
// computeTrustScore
// ---------------------------------------------------------------------------
describe("computeTrustScore", () => {
  it("returns 100 for no findings", () => {
    expect(computeTrustScore([])).toBe(100);
  });

  it("deducts 25 for a single Critical finding", () => {
    expect(computeTrustScore([finding("r1", "Critical")])).toBe(75);
  });

  it("deducts 15 for a single High finding", () => {
    expect(computeTrustScore([finding("r1", "High")])).toBe(85);
  });

  it("deducts 8 for a single Medium finding", () => {
    expect(computeTrustScore([finding("r1", "Medium")])).toBe(92);
  });

  it("deducts 3 for a single Low finding", () => {
    expect(computeTrustScore([finding("r1", "Low")])).toBe(97);
  });

  it("deducts only 1 for repeated hits of the same rule_id", () => {
    const findings = [
      finding("r1", "Critical"),
      finding("r1", "Critical"),
      finding("r1", "Critical"),
    ];
    // first hit: -25, second: -1, third: -1 => 100 - 27 = 73
    expect(computeTrustScore(findings)).toBe(73);
  });

  it("deducts independently for multiple different rules", () => {
    const findings = [
      finding("r1", "Critical"), // -25
      finding("r2", "High"), // -15
      finding("r3", "Medium"), // -8
    ];
    expect(computeTrustScore(findings)).toBe(100 - 25 - 15 - 8);
  });

  it("floors at 0", () => {
    const findings = [
      finding("a", "Critical"), // -25
      finding("b", "Critical"), // -25
      finding("c", "Critical"), // -25
      finding("d", "Critical"), // -25
      finding("e", "Critical"), // -25 => total -125, clamped to 0
    ];
    expect(computeTrustScore(findings)).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// maxSeverity
// ---------------------------------------------------------------------------
describe("maxSeverity", () => {
  it('returns "Low" for empty findings', () => {
    expect(maxSeverity([])).toBe("Low");
  });

  it("returns the highest severity present", () => {
    expect(maxSeverity([finding("r1", "Medium"), finding("r2", "Low")])).toBe(
      "Medium",
    );
    expect(maxSeverity([finding("r1", "High"), finding("r2", "Medium")])).toBe(
      "High",
    );
  });

  it('returns "Critical" when present', () => {
    expect(
      maxSeverity([
        finding("r1", "Low"),
        finding("r2", "Critical"),
        finding("r3", "High"),
      ]),
    ).toBe("Critical");
  });
});

// ---------------------------------------------------------------------------
// severityBadgeClass
// ---------------------------------------------------------------------------
describe("severityBadgeClass", () => {
  it("returns correct CSS class for Critical", () => {
    expect(severityBadgeClass("Critical")).toBe(
      "bg-trust-critical/10 text-trust-critical",
    );
  });

  it("returns correct CSS class for High", () => {
    expect(severityBadgeClass("High")).toBe(
      "bg-trust-high-risk/10 text-trust-high-risk font-semibold",
    );
  });

  it("returns correct CSS class for Medium", () => {
    expect(severityBadgeClass("Medium")).toBe(
      "bg-trust-low-risk/10 text-trust-low-risk",
    );
  });

  it("returns correct CSS class for Low", () => {
    expect(severityBadgeClass("Low")).toBe("bg-muted text-muted-foreground");
  });

  it("returns empty string for unknown severity", () => {
    expect(severityBadgeClass("Unknown")).toBe("");
  });
});

// ---------------------------------------------------------------------------
// severityIconColor
// ---------------------------------------------------------------------------
describe("severityIconColor", () => {
  it("returns correct color for Critical", () => {
    expect(severityIconColor("Critical")).toBe("text-trust-critical");
  });

  it("returns correct color for High", () => {
    expect(severityIconColor("High")).toBe("text-trust-high-risk");
  });

  it("returns correct color for Medium", () => {
    expect(severityIconColor("Medium")).toBe("text-trust-low-risk");
  });

  it("returns correct color for Low", () => {
    expect(severityIconColor("Low")).toBe("text-muted-foreground");
  });
});

// ---------------------------------------------------------------------------
// AUDIT_RULES
// ---------------------------------------------------------------------------
describe("AUDIT_RULES", () => {
  it("has unique rule IDs", () => {
    const ids = AUDIT_RULES.map((r) => r.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("all severities are valid", () => {
    const valid = new Set(["Critical", "High", "Medium", "Low"]);
    for (const rule of AUDIT_RULES) {
      expect(valid.has(rule.severity)).toBe(true);
    }
  });
});
