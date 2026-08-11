import { describe, expect, it } from "vitest";
import { createMockCatalog } from "../catalog/mock-catalog";
import {
  getDetailsReadiness,
  shouldShowEmptyScreenshots,
} from "./details-readiness";

const hydratedGame = createMockCatalog()[0];
if (!hydratedGame) throw new Error("Mock catalog has no games");

describe("Details readiness", () => {
  it("keeps a cold Details route closed until query data exists", () => {
    expect(getDetailsReadiness(undefined, "pending")).toBe("waiting");
    expect(getDetailsReadiness(hydratedGame, "success")).toBe("ready");
  });

  it("does not show a false empty screenshots state before hydration", () => {
    expect(shouldShowEmptyScreenshots(undefined, 0)).toBe(false);
    expect(shouldShowEmptyScreenshots(hydratedGame, 0)).toBe(true);
    expect(shouldShowEmptyScreenshots(hydratedGame, 1)).toBe(false);
  });
});
