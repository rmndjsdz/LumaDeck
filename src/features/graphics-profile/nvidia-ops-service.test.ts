import { describe, expect, it } from "vitest";
import { hasUsableNvidiaOpsProfile } from "./nvidia-ops-service";
import type { NvidiaOpsResponse } from "./nvidia-ops-types";

const profile = {
  source: "NVIDIA_OPTIMAL_PLAYABLE_SETTINGS" as const,
  sourceVersion: null,
  sourceFingerprint: "test",
  resolution: { width: 3840, height: 2160 },
  popIndex: 17,
  belowMinSpec: false,
  settings: [],
  confidence: "HIGH" as const,
};

function response(
  status: NvidiaOpsResponse["status"],
  withProfile: boolean,
): NvidiaOpsResponse {
  return {
    status,
    game: null,
    profile: withProfile ? profile : null,
    diagnostic: null,
  };
}

describe("NVIDIA OPS profile selection", () => {
  it("keeps AVAILABLE ahead of the LumaDeck fallback", () => {
    expect(hasUsableNvidiaOpsProfile(response("AVAILABLE", true))).toBe(true);
    expect(
      hasUsableNvidiaOpsProfile(response("BELOW_MIN_SPEC", true)),
    ).toBe(true);
  });

  it("falls back for GAME_NOT_FOUND and incomplete responses", () => {
    expect(hasUsableNvidiaOpsProfile(response("GAME_NOT_FOUND", false))).toBe(
      false,
    );
    expect(hasUsableNvidiaOpsProfile(response("AVAILABLE", false))).toBe(
      false,
    );
  });
});
