import { describe, expect, it } from "vitest";
import {
  capabilitySourceLabel,
  capabilityValueLabel,
} from "./game-capabilities-service";

describe("game capability labels", () => {
  it("keeps UNKNOWN visible as Desconocido", () => {
    expect(capabilityValueLabel("UNKNOWN")).toBe("Desconocido");
    expect(capabilityValueLabel("YES")).toBe("Sí");
    expect(capabilityValueLabel("NO")).toBe("No");
  });

  it("exposes the evidence source separately from the value", () => {
    expect(capabilitySourceLabel("PCGAMINGWIKI")).toBe("PCGamingWiki");
    expect(capabilitySourceLabel("USER_OVERRIDE")).toBe("Usuario");
    expect(capabilitySourceLabel("NONE")).toBe("Sin evidencia");
  });
});
