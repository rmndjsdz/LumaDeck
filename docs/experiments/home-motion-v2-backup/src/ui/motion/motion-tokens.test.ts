import { describe, expect, it } from "vitest";
import { motionTokens } from "./motion-tokens";

describe("motion tokens", () => {
  it("keeps visual timings inside the premium interaction budget", () => {
    expect(motionTokens.duration.instant).toBeGreaterThanOrEqual(80);
    expect(motionTokens.duration.instant).toBeLessThanOrEqual(90);
    expect(motionTokens.duration.focusFast).toBeGreaterThanOrEqual(120);
    expect(motionTokens.duration.focusFast).toBeLessThanOrEqual(160);
    expect(motionTokens.duration.standard).toBeGreaterThanOrEqual(170);
    expect(motionTokens.duration.standard).toBeLessThanOrEqual(215);
    expect(motionTokens.duration.backgroundCrossfade).toBeGreaterThanOrEqual(
      260,
    );
    expect(motionTokens.duration.backgroundCrossfade).toBeLessThanOrEqual(370);
    expect(motionTokens.duration.viewEnter).toBeGreaterThanOrEqual(260);
    expect(motionTokens.duration.viewEnter).toBeLessThanOrEqual(285);
    expect(motionTokens.duration.viewExit).toBeLessThan(
      motionTokens.duration.viewEnter,
    );
  });

  it("uses separate enter, exit, standard and focus curves", () => {
    expect(motionTokens.easing.standard).toBe("cubic-bezier(0.2, 0.8, 0.2, 1)");
    expect(motionTokens.easing.enter).toBe("cubic-bezier(0.16, 1, 0.3, 1)");
    expect(motionTokens.easing.exit).toBe("cubic-bezier(0.4, 0, 1, 1)");
    expect(motionTokens.easing.focus).toBe("cubic-bezier(0.18, 0.9, 0.25, 1)");
  });
});
