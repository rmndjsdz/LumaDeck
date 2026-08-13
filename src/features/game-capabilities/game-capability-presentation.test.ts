import { describe, expect, it } from "vitest";
import {
  capabilityStateClass,
  capabilityStateLabel,
} from "./game-capability-presentation";
import type { ResolvedCapability } from "./game-capabilities-types";

function capability(
  value: ResolvedCapability["value"],
  alternativeAvailable: ResolvedCapability["alternativeAvailable"],
): ResolvedCapability {
  return {
    kind: "NATIVE_HDR",
    value,
    confidence: "HIGH",
    source: "PCGAMINGWIKI",
    technologies: [],
    alternativeAvailable,
    sourceNote: null,
    evidence: null,
    otherEvidence: [],
    resolvedAt: 1,
    stale: false,
    hasConflict: false,
  };
}

describe("game capability presentation", () => {
  it("keeps native HDR NO as No compatible even with an alternative", () => {
    const result = capabilityStateLabel(capability("NO", "YES"));
    expect(result).toBe("No compatible");
    expect(capabilityStateClass(capability("NO", "YES"))).toBe("is-no");
  });

  it("preserves UNKNOWN as Desconocido", () => {
    expect(capabilityStateLabel(capability("UNKNOWN", "UNKNOWN"))).toBe(
      "Desconocido",
    );
  });
});
