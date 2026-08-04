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
  | "integrations"
  | "steam"
  | "hltb"
  | "steamgriddb"
  | "lossless-scaling"
  | "storage";

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
