import { afterEach, describe, expect, it, vi } from "vitest";
import { NavigationSettlingController } from "./navigation-settling";

describe("NavigationSettlingController", () => {
  afterEach(() => vi.useRealTimers());

  it("classifies repeated input as fast navigation and settles the final target", () => {
    vi.useFakeTimers();
    const phases: string[] = [];
    const controller = new NavigationSettlingController({
      onPhaseChange: (phase) => phases.push(phase),
      settleAfterMs: 100,
      idleAfterMs: 120,
      fastThresholdMs: 160,
    });

    controller.notifyNavigation(0);
    expect(controller.getPhase()).toBe("navigating");
    controller.notifyNavigation(50);
    expect(controller.getPhase()).toBe("fast-navigating");
    vi.advanceTimersByTime(100);
    expect(controller.getPhase()).toBe("settling");
    vi.advanceTimersByTime(120);
    expect(controller.getPhase()).toBe("idle");
    expect(phases).toEqual([
      "navigating",
      "fast-navigating",
      "settling",
      "idle",
    ]);
    controller.dispose();
  });

  it("cancels the previous settling timer when navigation continues", () => {
    vi.useFakeTimers();
    const phases: string[] = [];
    const controller = new NavigationSettlingController({
      onPhaseChange: (phase) => phases.push(phase),
      settleAfterMs: 100,
    });

    controller.notifyNavigation(0);
    vi.advanceTimersByTime(80);
    controller.notifyNavigation(80);
    vi.advanceTimersByTime(99);
    expect(phases).toEqual(["navigating", "fast-navigating"]);
    vi.advanceTimersByTime(1);
    expect(phases).toContain("settling");
    controller.dispose();
  });
});
