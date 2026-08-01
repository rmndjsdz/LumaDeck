import { describe, expect, it } from "vitest";

import { findSpatialCandidate } from "./spatial-navigation";
import type { Rect } from "./navigation-types";

const rect = (left: number, top: number, width = 80, height = 40): Rect => ({
  left,
  top,
  width,
  height,
  right: left + width,
  bottom: top + height,
});

describe("findSpatialCandidate", () => {
  it("resolves each cardinal direction", () => {
    const current = rect(100, 100);
    const candidates = [
      { focusId: "up", rect: rect(100, 20) },
      { focusId: "down", rect: rect(100, 180) },
      { focusId: "left", rect: rect(0, 100) },
      { focusId: "right", rect: rect(200, 100) },
    ];

    expect(
      findSpatialCandidate(current, candidates, "up").candidate?.focusId,
    ).toBe("up");
    expect(
      findSpatialCandidate(current, candidates, "down").candidate?.focusId,
    ).toBe("down");
    expect(
      findSpatialCandidate(current, candidates, "left").candidate?.focusId,
    ).toBe("left");
    expect(
      findSpatialCandidate(current, candidates, "right").candidate?.focusId,
    ).toBe("right");
  });

  it("prefers aligned candidates and resolves ties deterministically", () => {
    const result = findSpatialCandidate(
      rect(100, 100),
      [
        { focusId: "zeta", rect: rect(220, 110) },
        { focusId: "alpha", rect: rect(220, 110) },
        { focusId: "aligned", rect: rect(220, 100) },
      ],
      "right",
    );

    expect(result.candidate?.focusId).toBe("aligned");
    expect(result.evaluated).toContain("alpha");
  });

  it("omits disabled and hidden candidates", () => {
    const result = findSpatialCandidate(
      rect(0, 0),
      [
        { focusId: "disabled", rect: rect(100, 0), disabled: true },
        { focusId: "hidden", rect: rect(120, 0), hidden: true },
      ],
      "right",
    );

    expect(result.candidate).toBeNull();
    expect(result.evaluated).toEqual([]);
  });
});
