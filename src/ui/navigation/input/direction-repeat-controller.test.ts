import { describe, expect, it, vi } from "vitest";

import { DirectionRepeatController } from "./direction-repeat-controller";

describe("DirectionRepeatController", () => {
  it("delays, repeats, accelerates, and cancels centrally", () => {
    vi.useFakeTimers();
    const callback = vi.fn();
    const controller = new DirectionRepeatController({
      initialDelayMs: 20,
      intervalMs: 10,
      acceleratedIntervalMs: 5,
      accelerationAfter: 2,
    });

    controller.start("down", callback);
    expect(callback).not.toHaveBeenCalled();
    vi.advanceTimersByTime(20);
    expect(callback).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(10);
    expect(callback).toHaveBeenCalledTimes(2);
    controller.stop();
    vi.advanceTimersByTime(50);
    expect(callback).toHaveBeenCalledTimes(2);
    vi.useRealTimers();
  });
});
