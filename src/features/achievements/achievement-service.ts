import { invoke } from "@tauri-apps/api/core";
import {
  emptyGameAchievements,
  type AchievementDistribution,
  type AchievementSummary,
  type GameAchievements,
} from "./achievement-types";

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const achievementsService = {
  getGameAchievements: async (gameId: string): Promise<GameAchievements> => {
    if (!isTauriRuntime()) return emptyGameAchievements(gameId);
    return invoke<GameAchievements>("get_game_achievements", { gameId });
  },
  refreshGameAchievements: async (
    gameId: string,
  ): Promise<GameAchievements> => {
    if (!isTauriRuntime()) return emptyGameAchievements(gameId);
    return invoke<GameAchievements>("refresh_game_achievements", { gameId });
  },
  getAchievementSummary: async (
    gameId: string,
  ): Promise<AchievementSummary> => {
    if (!isTauriRuntime()) return emptyGameAchievements(gameId).summary;
    return invoke<AchievementSummary>("get_achievement_summary", { gameId });
  },
  getAchievementDistribution: async (
    gameId: string,
  ): Promise<AchievementDistribution> => {
    if (!isTauriRuntime()) return emptyGameAchievements(gameId).distribution;
    return invoke<AchievementDistribution>("get_achievement_distribution", {
      gameId,
    });
  },
};
