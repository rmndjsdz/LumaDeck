export type GameStatus = "not-started" | "playing" | "completed";

export type SteamGameDetails = {
  appId: number;
  name: string;
  totalPlaytimeMinutes: number;
  playtime2WeeksMinutes: number | null;
  lastPlayedAt: string | null;
  installed: boolean | null;
  owned: boolean | null;
  hidden: boolean | null;
  acquiredAt: string | null;
  achievementTotal: number | null;
  achievementUnlocked: number | null;
  achievementProgress: number | null;
  stats: Record<string, unknown>;
  tags: string[];
  genres: string[];
  categories: string[];
  developers: string[];
  publishers: string[];
  languages: string[];
  platforms: string[];
  controllerSupport: string | null;
  releaseDate: string | null;
  description: string | null;
  shortDescription: string | null;
  website: string | null;
  minimumRequirements: unknown | null;
  recommendedRequirements: unknown | null;
  headerUrl: string | null;
  screenshots: string[];
  movies: string[];
  reviewScore: number | null;
  reviewCount: number | null;
  price: unknown | null;
  dlc: number[];
  earlyAccess: boolean | null;
  adultContent: boolean | null;
  multiplayer: boolean | null;
  singlePlayer: boolean | null;
  cloud: boolean | null;
  tradingCards: boolean | null;
  workshop: boolean | null;
  familySharing: boolean | null;
};

export type SteamGameMetrics = {
  totalPlaytimeMinutes: number;
  lastPlayedAt: string | null;
  progress: number;
  achievementTotal: number | null;
  achievementUnlocked: number | null;
  activePlayers: number | null;
};

export type HltbGameData = {
  gameId: string;
  hltbId: string | null;
  matchedTitle: string | null;
  mainStoryMinutes: number | null;
  mainExtraMinutes: number | null;
  completionistMinutes: number | null;
  matchConfidence: number | null;
  matchType: string | null;
  lastSyncedAt: string | null;
  source: string;
  status: "matched" | "unmatched" | "error" | string;
  lastError: string | null;
};

export type GameDetails = {
  steam?: SteamGameDetails;
  hltb?: HltbGameData;
};

export type GameAchievementSummary = {
  total: number | null;
  unlocked: number | null;
  progress: number | null;
};

export type Game = {
  id: string;
  title: string;
  sortTitle: string;
  platform: string;
  provider: string;
  coverUrl: string;
  verticalCoverUrl: string;
  squareCoverUrl?: string;
  logoUrl: string;
  backgroundUrl: string;
  iconUrl?: string;
  screenshots: string[];
  description: string;
  genres: string[];
  releaseYear: number;
  playtimeMinutes: number;
  lastPlayedAt: string | null;
  favorite: boolean;
  installed: boolean;
  progress: number;
  status: GameStatus;
  achievements?: GameAchievementSummary;
  details?: GameDetails;
};
