export type AchievementRarity =
  "very-rare" | "rare" | "uncommon" | "common" | "very-common";

export type AchievementVirtualTier = "bronze" | "silver" | "gold";

export type Achievement = {
  apiName: string;
  displayName: string;
  description: string;
  hidden: boolean;
  unlocked: boolean;
  unlockTime: string | null;
  unlockPercentage: number | null;
  rarity: AchievementRarity;
  virtualTier: AchievementVirtualTier;
  iconUnlocked: string | null;
  iconLocked: string | null;
  localIconUnlocked: string | null;
  localIconLocked: string | null;
};

export type AchievementSummary = {
  total: number;
  unlocked: number;
  locked: number;
  completionPercentage: number;
};

export type AchievementDistribution = {
  bronze: number;
  silver: number;
  gold: number;
};

export type AchievementRecent = {
  achievements: Achievement[];
};

export type GameAchievements = {
  gameId: string;
  steamAppId: number;
  achievements: Achievement[];
  summary: AchievementSummary;
  distribution: AchievementDistribution;
  recent: AchievementRecent;
  lastSyncedAt: string | null;
  syncStatus: string;
  schemaVersion: number;
};

export function emptyGameAchievements(gameId: string): GameAchievements {
  return {
    gameId,
    steamAppId: 0,
    achievements: [],
    summary: {
      total: 0,
      unlocked: 0,
      locked: 0,
      completionPercentage: 0,
    },
    distribution: { bronze: 0, silver: 0, gold: 0 },
    recent: { achievements: [] },
    lastSyncedAt: null,
    syncStatus: "unavailable",
    schemaVersion: 1,
  };
}
