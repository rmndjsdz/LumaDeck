import { describe, expect, it } from "vitest";
import { useGameSessionStore } from "./game-session-store";

describe("game session store", () => {
  it("freezes input for every backend-active state", () => {
    const applyStatus = useGameSessionStore.getState().applyStatus;
    for (const state of [
      "preparing",
      "launching",
      "running",
      "finishing",
    ] as const) {
      applyStatus({
        sessionId: "session-1",
        gameId: "game-1",
        steamAppId: 123,
        state,
        occurredAt: "100",
        elapsedSeconds: 4,
        message: "state",
      });
      expect(useGameSessionStore.getState().inputFrozen).toBe(true);
    }
  });

  it("releases input for idle and recoverable terminal states", () => {
    const applyStatus = useGameSessionStore.getState().applyStatus;
    for (const state of ["idle", "error", "unsupported"] as const) {
      applyStatus({
        sessionId: "session-1",
        gameId: "game-1",
        steamAppId: 123,
        state,
        occurredAt: "100",
        elapsedSeconds: 4,
        message: "state",
      });
      expect(useGameSessionStore.getState().inputFrozen).toBe(false);
    }
  });
});
