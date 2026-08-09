import { describe, expect, it } from "vitest";

import {
  clampScreenshotPan,
  getCircularScreenshotIndex,
  getZoomAfterStep,
} from "./screenshot-viewer-state";

describe("screenshot viewer state", () => {
  it("wraps screenshot navigation in both directions", () => {
    expect(getCircularScreenshotIndex(0, 6, -1)).toBe(5);
    expect(getCircularScreenshotIndex(5, 6, 1)).toBe(0);
  });

  it("clamps zoom to the supported 100%-300% range", () => {
    expect(getZoomAfterStep(100, -1)).toBe(100);
    expect(getZoomAfterStep(100, 1)).toBe(150);
    expect(getZoomAfterStep(300, 1)).toBe(300);
  });

  it("keeps pan inside the available viewport bounds", () => {
    expect(clampScreenshotPan({ x: 120, y: -80 }, { x: 64, y: 48 })).toEqual({
      x: 64,
      y: -48,
    });
  });
});
