import { describe, expect, it } from "vitest";
import { achievementsService } from "./achievement-service";

describe("achievementsService", () => {
  it("returns a stable empty result outside the Tauri desktop runtime", async () => {
    const result = await achievementsService.getGameAchievements("game-001");

    expect(result.gameId).toBe("game-001");
    expect(result.achievements).toEqual([]);
    expect(result.summary).toEqual({
      total: 0,
      unlocked: 0,
      locked: 0,
      completionPercentage: 0,
    });
    expect(result.syncStatus).toBe("unavailable");
  });
});
