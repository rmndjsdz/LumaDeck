import { invoke } from "@tauri-apps/api/core";
import {
  emptyGameAchievements,
  type AchievementDistributions,
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
    force = true,
  ): Promise<GameAchievements> => {
    if (!isTauriRuntime()) return emptyGameAchievements(gameId);
    return invoke<GameAchievements>("refresh_game_achievements", {
      gameId,
      force,
    });
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
  getAchievementDistributions: async (
    gameId: string,
  ): Promise<AchievementDistributions> => {
    if (!isTauriRuntime()) {
      const empty = emptyGameAchievements(gameId);
      return {
        total: empty.totalDistribution,
        unlocked: empty.unlockedDistribution,
      };
    }
    return invoke<AchievementDistributions>("get_achievement_distributions", {
      gameId,
    });
  },
  cancelRefreshGameAchievements: async (): Promise<void> => {
    if (!isTauriRuntime()) return;
    await invoke<void>("cancel_game_achievements_refresh");
  },
};
