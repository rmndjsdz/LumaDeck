export type SteamConfigurationState =
  | "loading"
  | "not-configured"
  | "partially-configured"
  | "configured"
  | "saving"
  | "save-error"
  | "credential-unavailable";

export type SteamProfileState =
  "idle" | "loading" | "loaded" | "offline" | "error";

export interface SteamConfigurationStatus {
  providerId: "steam";
  accountId?: string;
  steamId64Masked?: string;
  apiKeyConfigured: boolean;
  apiKeyMasked?: string;
  status: Exclude<SteamConfigurationState, "loading" | "saving" | "save-error">;
}

export type SteamGridDbConfigurationState =
  "not-configured" | "configured" | "credential-unavailable";

export interface SteamGridDbConfigurationStatus {
  providerId: "steamgriddb";
  apiKeyConfigured: boolean;
  apiKeyMasked?: string;
  credentialAvailable: boolean;
  status: SteamGridDbConfigurationState;
  enabled: boolean;
}

export type ArtworkEnrichmentScope = "only_non_steam" | "all";
export type ArtworkEnrichmentSlot =
  "grid_horizontal" | "grid_vertical" | "grid_square" | "hero" | "logo";

export interface ArtworkEnrichmentRequest {
  gameIds: string[];
  scope: ArtworkEnrichmentScope;
  slots: ArtworkEnrichmentSlot[];
  maxDimension: number;
  concurrency: number;
}

export interface ArtworkEnrichmentStatus {
  status: "idle" | "running" | "completed" | "cancelled" | "error" | string;
  processedGames: number;
  totalGames: number;
  currentGame?: string | null;
  currentArtwork?: string | null;
  downloadedAssets: number;
  alreadyCompleteGames: number;
  noResultGames: number;
  ambiguousGames: number;
  errorCount: number;
  durationMs?: number | null;
  startedAt?: string | null;
  completedAt?: string | null;
  errorMessage?: string | null;
}

export type RapidApiReviewsConfigurationState =
  "not-configured" | "configured" | "credential-unavailable";

export interface RapidApiReviewsConfigurationStatus {
  providerId: "rapidapi-reviews";
  apiKeyConfigured: boolean;
  apiKeyMasked?: string;
  credentialAvailable: boolean;
  status: RapidApiReviewsConfigurationState;
  enabled: boolean;
}

export type AIConnectionState =
  | "not-configured"
  | "configured"
  | "connecting"
  | "connected"
  | "authentication-error"
  | "offline"
  | "timeout"
  | "invalid-model"
  | "error"
  | "credential-unavailable";

export interface AIConfiguration {
  providerId: "openrouter";
  model: string;
}

export interface AIConnectionStatus {
  state: AIConnectionState;
  message?: string;
}

export interface AIConfigurationStatus {
  configuration: AIConfiguration;
  apiKeyConfigured: boolean;
  apiKeyMasked?: string;
  credentialAvailable: boolean;
  connection: AIConnectionStatus;
}

export interface SteamProfile {
  steamId64: string;
  avatarUrl: string;
  personaName: string;
  countryCode?: string;
  gameCount: number;
}

export type SteamLibrarySyncScope = "all" | "installed";

export interface SteamLibrarySyncSettings {
  scope: SteamLibrarySyncScope;
}

export interface DatabaseStatus {
  path: string;
  schemaVersion: number;
  providerCount: number;
}

export type SteamSyncStatus = {
  status: "idle" | "running" | "completed" | "cancelled" | "error";
  foundCount: number;
  createdCount: number;
  updatedCount: number;
  progressCompleted: number;
  progressTotal: number;
  durationMs?: number;
  startedAt?: string;
  completedAt?: string;
  currentAppId?: number;
  errorMessage?: string;
};

export type SteamSyncResult = Pick<
  SteamSyncStatus,
  | "status"
  | "foundCount"
  | "createdCount"
  | "updatedCount"
  | "durationMs"
  | "completedAt"
>;

export type SteamImageSyncStatus = {
  status: "idle" | "running" | "completed" | "cancelled" | "error";
  foundCount: number;
  downloadedCount: number;
  skippedCount: number;
  progressCompleted: number;
  progressTotal: number;
  durationMs?: number;
  startedAt?: string;
  completedAt?: string;
  currentAppId?: number;
  errorMessage?: string;
};

export type SteamImageSyncResult = Pick<
  SteamImageSyncStatus,
  | "status"
  | "foundCount"
  | "downloadedCount"
  | "skippedCount"
  | "durationMs"
  | "completedAt"
>;

export type SteamAchievementSyncStatus = {
  status: "idle" | "running" | "completed" | "error";
  foundCount: number;
  updatedCount: number;
  skippedCount: number;
  durationMs?: number;
  completedAt?: string;
};

export type SteamAchievementSyncResult = Pick<
  SteamAchievementSyncStatus,
  | "status"
  | "foundCount"
  | "updatedCount"
  | "skippedCount"
  | "durationMs"
  | "completedAt"
>;

export interface HltbSettings {
  enabled: boolean;
  syncWithSteam: boolean;
  showMainStory: boolean;
  showMainExtra: boolean;
  showCompletionist: boolean;
}

export type HltbSyncStatus = {
  status: "idle" | "running" | "completed" | "cancelled" | "error";
  processedCount: number;
  totalCount: number;
  foundCount: number;
  unmatchedCount: number;
  exactMatchCount: number;
  approximateMatchCount: number;
  errorCount: number;
  durationMs?: number;
  startedAt?: string;
  completedAt?: string;
  lastError?: string;
};

export type HltbCandidate = {
  hltbId: string;
  title: string;
  mainStoryMinutes?: number;
  mainExtraMinutes?: number;
  completionistMinutes?: number;
};

export type HltbPendingMatch = {
  gameId: string;
  title: string;
  aliasQuery?: string;
  resolutionStatus?: string;
};

export type SettingsLevel =
  | "settings"
  | "appearance"
  | "display"
  | "network"
  | "bluetooth"
  | "library"
  | "integrations"
  | "steam"
  | "hltb"
  | "steamgriddb"
  | "rapidapi-reviews"
  | "ai-services"
  | "lossless-scaling"
  | "launchbox"
  | "storage"
  | "eden";

export type EdenLibraryRootStatus = {
  path: string;
  deepScan: boolean;
  available: boolean;
  gameCount: number;
  error: string | null;
};

export type EdenProfile = {
  id: string;
  name: string;
  avatarDataUrl: string | null;
  isCurrent: boolean;
};

export type EdenStatus = {
  providerId: "eden";
  status: "not-configured" | "ready" | "configuration-missing" | string;
  executablePath: string | null;
  dataPath: string | null;
  configPath: string | null;
  portable: boolean;
  configurationFound: boolean;
  libraryRoots: EdenLibraryRootStatus[];
  profiles: EdenProfile[];
  gamesDetected: number;
  duplicateGames: number;
  playtimeSynced: number;
  playtimeUnavailable: number;
  playtimeFileFound: boolean;
  warnings: string[];
};

export type EdenExecutableInspection = {
  executablePath: string;
  valid: boolean;
  dataPath: string | null;
  configPath: string | null;
  portable: boolean;
  configurationFound: boolean;
  libraryRoots: { path: string; deepScan: boolean }[];
  profiles: EdenProfile[];
  warnings: string[];
};

export type StorageMigrationStatus = {
  status: "idle" | "running" | "completed" | "error";
  currentMode: "appData" | "portable" | string;
  currentPath: string;
  targetMode?: "appData" | "portable" | string;
  targetPath?: string;
  filesCopied: number;
  totalFiles: number;
  bytesCopied: number;
  totalBytes: number;
  errorMessage?: string;
  needsRestart: boolean;
  deleteSource: boolean;
};

export interface StorageStatus {
  mode: "appData" | "portable" | string;
  currentPath: string;
  normalPath: string;
  portablePath: string;
  usedBytes: number;
  migration: StorageMigrationStatus;
}

export interface StorageMigrationResult {
  status: "completed";
  sourceMode: string;
  targetMode: string;
  sourcePath: string;
  targetPath: string;
  filesCopied: number;
  bytesCopied: number;
  needsRestart: boolean;
}
