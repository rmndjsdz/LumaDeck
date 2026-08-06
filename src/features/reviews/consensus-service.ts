import { invoke } from "@tauri-apps/api/core";
import type { GameReviewConsensus } from "./consensus-types";

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const consensusService = {
  async get(gameId: string): Promise<GameReviewConsensus | null> {
    if (!isTauriRuntime()) return null;
    return invoke<GameReviewConsensus | null>("get_game_review_consensus", {
      gameId,
    });
  },
  async generate(
    gameId: string,
    forceRefresh: boolean,
  ): Promise<GameReviewConsensus> {
    if (!isTauriRuntime()) {
      throw new Error("CONSENSUS_RUNTIME_UNAVAILABLE");
    }
    return invoke<GameReviewConsensus>("generate_game_review_consensus", {
      gameId,
      forceRefresh,
    });
  },
};
