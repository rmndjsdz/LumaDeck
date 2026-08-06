import { invoke } from "@tauri-apps/api/core";
import type { SteamGameMetrics } from "../catalog/game-types";
import type {
  DatabaseStatus,
  SteamProfile,
  SteamConfigurationStatus,
  SteamSyncResult,
  SteamSyncStatus,
  SteamImageSyncResult,
  SteamImageSyncStatus,
  SteamAchievementSyncResult,
  SteamLibrarySyncSettings,
  HltbSettings,
  HltbCandidate,
  HltbPendingMatch,
  HltbSyncStatus,
  SteamGridDbConfigurationStatus,
  RapidApiReviewsConfigurationStatus,
  AIConfigurationStatus,
  AIConnectionStatus,
  StorageMigrationResult,
  StorageStatus,
} from "./settings-types";

export type SettingsErrorCategory =
  | "IPC_INVALID_ARGUMENTS"
  | "IPC_COMMAND_NOT_FOUND"
  | "DATABASE_ERROR"
  | "VALIDATION_ERROR"
  | "CREDENTIAL_ERROR"
  | "STEAM_OFFLINE"
  | "STEAM_API_ERROR"
  | "STEAM_INVALID_RESPONSE"
  | "STEAM_SYNC_ALREADY_RUNNING"
  | "STEAM_SYNC_CANCELLED"
  | "STEAM_INSTALLED_GAMES_UNAVAILABLE"
  | "STEAM_IMAGE_SYNC_ALREADY_RUNNING"
  | "STEAM_IMAGE_SYNC_CANCELLED"
  | "STEAM_IMAGE_DOWNLOAD_FAILED"
  | "STEAM_ACHIEVEMENT_SYNC_ALREADY_RUNNING"
  | "GAME_NOT_FOUND"
  | "STEAM_METADATA_NOT_AVAILABLE"
  | "STEAM_METADATA_SYNC_ALREADY_RUNNING"
  | "HLTB_SYNC_ALREADY_RUNNING"
  | "HLTB_SYNC_CANCELLED"
  | "HLTB_DISABLED"
  | "HLTB_OFFLINE"
  | "HLTB_API_ERROR"
  | "HLTB_INVALID_RESPONSE"
  | "STORAGE_MIGRATION_ALREADY_RUNNING"
  | "STORAGE_MIGRATION_BUSY"
  | "STORAGE_MIGRATION_SAME_MODE"
  | "STORAGE_MIGRATION_STATE_UNAVAILABLE"
  | "STORAGE_MIGRATION_IO_ERROR"
  | "STORAGE_MIGRATION_DATABASE_INVALID"
  | "STORAGE_MIGRATION_VALIDATION_ERROR"
  | "STORAGE_MIGRATION_INVALID_MODE"
  | "INVALID_AI_PROVIDER"
  | "INVALID_AI_MODEL"
  | "INVALID_AI_API_KEY"
  | "UNKNOWN_ERROR";

export class ProviderSettingsError extends Error {
  public readonly code: SettingsErrorCategory;
  public readonly diagnostic: string | undefined;
  public readonly backendCode: string | undefined;

  public constructor(
    code: SettingsErrorCategory,
    diagnostic?: string,
    backendCode?: string,
  ) {
    super(code);
    this.name = "ProviderSettingsError";
    this.code = code;
    this.diagnostic = diagnostic;
    this.backendCode = backendCode;
  }
}

export type SettingsSaveCorrelationId = `settings-save-${string}`;

export function createSettingsSaveCorrelationId(): SettingsSaveCorrelationId {
  const uuid = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}`;
  return `settings-save-${uuid}`;
}

export function logSettingsDiagnostic(
  event: string,
  details: Record<string, unknown>,
): void {
  if (import.meta.env.DEV) {
    console.debug(`[settings] ${event}`, details);
  }
}

function safeDiagnosticText(value: string): string {
  return value
    .replace(/\b\d{17}\b/g, "<steamid64>")
    .replace(/\b[A-Za-z0-9_-]{16,64}\b/g, "<redacted-token>");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

function describeError(error: unknown): Record<string, unknown> {
  const record = isRecord(error) ? error : null;
  const message =
    error instanceof Error
      ? error.message
      : record && typeof record.message === "string"
        ? record.message
        : typeof error === "string"
          ? error
          : undefined;
  const code = record?.code;
  return {
    typeof: typeof error,
    name: error instanceof Error ? error.name : record?.name,
    message:
      typeof message === "string" ? safeDiagnosticText(message) : undefined,
    code: typeof code === "string" ? code : undefined,
    objectKeys: record ? Object.keys(record).sort() : undefined,
    stringLength: typeof error === "string" ? error.length : undefined,
  };
}

function payloadShape(args?: Record<string, unknown>): Record<string, unknown> {
  const steamId64 = args?.steamId64;
  const apiKey = args?.apiKey;
  const correlationId = args?.correlationId;
  const model = args?.model;
  return {
    steamId64Length:
      typeof steamId64 === "string" ? steamId64.trim().length : undefined,
    apiKeyPresent:
      typeof apiKey === "string" ? apiKey.trim().length > 0 : undefined,
    providerId:
      typeof args?.providerId === "string" ? args.providerId : undefined,
    model: typeof model === "string" ? model : undefined,
    accountIdPresent:
      typeof args?.accountId === "string" && args.accountId.length > 0,
    correlationId:
      typeof correlationId === "string" ? correlationId : undefined,
  };
}

function classifyError(
  rawCode: string | undefined,
  message: string | undefined,
): SettingsErrorCategory {
  const normalized = `${rawCode ?? ""} ${message ?? ""}`.toLowerCase();
  if (
    normalized.includes("invalid args") ||
    normalized.includes("missing required key") ||
    normalized.includes("unexpected argument")
  ) {
    return "IPC_INVALID_ARGUMENTS";
  }
  if (
    normalized.includes("command not found") ||
    normalized.includes("unknown command") ||
    normalized.includes("not registered")
  ) {
    return "IPC_COMMAND_NOT_FOUND";
  }
  if (rawCode === "DATABASE_ERROR" || rawCode === "DATABASE_PATH_UNAVAILABLE") {
    return "DATABASE_ERROR";
  }
  if (
    rawCode === "INVALID_STEAM_ID" ||
    rawCode === "INVALID_API_KEY" ||
    rawCode === "ACCOUNT_NOT_CONFIGURED"
  ) {
    return "VALIDATION_ERROR";
  }
  if (rawCode === "CREDENTIAL_UNAVAILABLE") return "CREDENTIAL_ERROR";
  if (rawCode === "STEAM_OFFLINE") return "STEAM_OFFLINE";
  if (rawCode === "STEAM_API_ERROR") return "STEAM_API_ERROR";
  if (rawCode === "STEAM_INVALID_RESPONSE") return "STEAM_INVALID_RESPONSE";
  if (rawCode === "STEAM_SYNC_ALREADY_RUNNING")
    return "STEAM_SYNC_ALREADY_RUNNING";
  if (rawCode === "STEAM_SYNC_CANCELLED") return "STEAM_SYNC_CANCELLED";
  if (rawCode === "STEAM_INSTALLED_GAMES_UNAVAILABLE")
    return "STEAM_INSTALLED_GAMES_UNAVAILABLE";
  if (rawCode === "STEAM_IMAGE_SYNC_ALREADY_RUNNING")
    return "STEAM_IMAGE_SYNC_ALREADY_RUNNING";
  if (rawCode === "STEAM_IMAGE_SYNC_CANCELLED")
    return "STEAM_IMAGE_SYNC_CANCELLED";
  if (rawCode === "STEAM_IMAGE_DOWNLOAD_FAILED")
    return "STEAM_IMAGE_DOWNLOAD_FAILED";
  if (rawCode === "STEAM_ACHIEVEMENT_SYNC_ALREADY_RUNNING")
    return "STEAM_ACHIEVEMENT_SYNC_ALREADY_RUNNING";
  if (rawCode === "GAME_NOT_FOUND") return "GAME_NOT_FOUND";
  if (rawCode === "STEAM_METADATA_NOT_AVAILABLE")
    return "STEAM_METADATA_NOT_AVAILABLE";
  if (rawCode === "STEAM_METADATA_SYNC_ALREADY_RUNNING")
    return "STEAM_METADATA_SYNC_ALREADY_RUNNING";
  if (rawCode === "HLTB_SYNC_ALREADY_RUNNING")
    return "HLTB_SYNC_ALREADY_RUNNING";
  if (rawCode === "HLTB_SYNC_CANCELLED") return "HLTB_SYNC_CANCELLED";
  if (rawCode === "HLTB_DISABLED") return "HLTB_DISABLED";
  if (rawCode === "HLTB_OFFLINE") return "HLTB_OFFLINE";
  if (rawCode === "HLTB_API_ERROR") return "HLTB_API_ERROR";
  if (rawCode === "HLTB_INVALID_RESPONSE") return "HLTB_INVALID_RESPONSE";
  if (rawCode === "STORAGE_MIGRATION_ALREADY_RUNNING")
    return "STORAGE_MIGRATION_ALREADY_RUNNING";
  if (rawCode === "STORAGE_MIGRATION_BUSY") return "STORAGE_MIGRATION_BUSY";
  if (rawCode === "STORAGE_MIGRATION_SAME_MODE")
    return "STORAGE_MIGRATION_SAME_MODE";
  if (rawCode === "STORAGE_MIGRATION_STATE_UNAVAILABLE")
    return "STORAGE_MIGRATION_STATE_UNAVAILABLE";
  if (rawCode === "STORAGE_MIGRATION_IO_ERROR")
    return "STORAGE_MIGRATION_IO_ERROR";
  if (rawCode === "STORAGE_MIGRATION_DATABASE_INVALID")
    return "STORAGE_MIGRATION_DATABASE_INVALID";
  if (rawCode === "STORAGE_MIGRATION_VALIDATION_ERROR")
    return "STORAGE_MIGRATION_VALIDATION_ERROR";
  if (rawCode === "STORAGE_MIGRATION_INVALID_MODE")
    return "STORAGE_MIGRATION_INVALID_MODE";
  if (rawCode === "INVALID_AI_PROVIDER") return "INVALID_AI_PROVIDER";
  if (rawCode === "INVALID_AI_MODEL") return "INVALID_AI_MODEL";
  if (rawCode === "INVALID_AI_API_KEY") return "INVALID_AI_API_KEY";
  return "UNKNOWN_ERROR";
}

function responseType(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}

function mapError(
  error: unknown,
  correlationId: string | undefined,
): ProviderSettingsError {
  const description = describeError(error);
  logSettingsDiagnostic("INVOKE_REJECTED", {
    correlationId,
    error: description,
  });
  const rawCode =
    typeof error === "string"
      ? error
      : error !== null && typeof error === "object" && "code" in error
        ? error.code
        : null;
  const typedRawCode = typeof rawCode === "string" ? rawCode : undefined;
  const rawMessage =
    typeof description.message === "string" ? description.message : undefined;
  const code = classifyError(typedRawCode, rawMessage);
  logSettingsDiagnostic("ERROR_MAPPED", {
    correlationId,
    code,
    backendCode: typedRawCode,
  });
  const diagnostic = Object.entries(description)
    .filter(([, value]) => value !== undefined)
    .map(([key, value]) => `${key}=${String(value)}`)
    .join("; ");
  return new ProviderSettingsError(code, diagnostic, typedRawCode);
}

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
  correlationId?: string,
): Promise<T> {
  logSettingsDiagnostic("INVOKE_BEGIN", {
    correlationId,
    command,
    payload: payloadShape(args),
  });
  try {
    const response = await invoke<T>(command, args);
    logSettingsDiagnostic("INVOKE_RESOLVED", {
      correlationId,
      command,
      responseType: responseType(response),
    });
    return response;
  } catch (error) {
    throw mapError(error, correlationId);
  }
}

export const providerSettingsService = {
  getAIConfiguration: () => call<AIConfigurationStatus>("get_ai_configuration"),
  saveAIConfiguration: (providerId: string, model: string, apiKey: string) =>
    call<AIConfigurationStatus>("save_ai_configuration", {
      providerId,
      model,
      apiKey,
    }),
  testAIConnection: (providerId: string, model: string, apiKey: string) =>
    call<AIConnectionStatus>("test_ai_connection", {
      providerId,
      model,
      apiKey,
    }),
  getSteamConfiguration: () =>
    call<SteamConfigurationStatus>("get_provider_configuration", {
      providerId: "steam",
    }),
  saveSteamConfiguration: (
    steamId64: string,
    apiKey: string,
    correlationId?: SettingsSaveCorrelationId,
  ) =>
    call<SteamConfigurationStatus>(
      "save_steam_account_configuration",
      {
        steamId64,
        apiKey,
        correlationId,
      },
      correlationId,
    ),
  updateSteamId: (
    steamId64: string,
    correlationId?: SettingsSaveCorrelationId,
  ) =>
    call<SteamConfigurationStatus>(
      "update_steam_id",
      {
        steamId64,
        correlationId,
      },
      correlationId,
    ),
  replaceSteamApiKey: (
    apiKey: string,
    correlationId?: SettingsSaveCorrelationId,
  ) =>
    call<SteamConfigurationStatus>(
      "replace_steam_api_key",
      {
        apiKey,
        correlationId,
      },
      correlationId,
    ),
  disconnect: (accountId: string) =>
    call<SteamConfigurationStatus>("disconnect_provider_account", {
      accountId,
    }),
  getDatabaseStatus: () => call<DatabaseStatus>("get_database_status"),
  getSteamProfile: () => call<SteamProfile>("get_steam_profile"),
  getSteamSyncStatus: () => call<SteamSyncStatus>("get_steam_sync_status"),
  getSteamLibrarySyncSettings: () =>
    call<SteamLibrarySyncSettings>("get_steam_library_sync_settings"),
  setSteamLibrarySyncScope: (scope: "all" | "installed") =>
    call<SteamLibrarySyncSettings>("set_steam_library_sync_scope", { scope }),
  syncSteamLibrary: () => call<SteamSyncResult>("sync_steam_library"),
  cancelSteamLibrarySync: () =>
    call<SteamSyncStatus>("cancel_steam_library_sync"),
  getSteamImageSyncStatus: () =>
    call<SteamImageSyncStatus>("get_steam_image_sync_status"),
  syncSteamImages: () => call<SteamImageSyncResult>("sync_steam_images"),
  syncSteamAchievements: () =>
    call<SteamAchievementSyncResult>("sync_steam_achievements"),
  getHltbSettings: () => call<HltbSettings>("get_hltb_settings"),
  setHltbSettings: (settings: HltbSettings) =>
    call<HltbSettings>("set_hltb_settings", { settings }),
  getHltbSyncStatus: () => call<HltbSyncStatus>("get_hltb_sync_status"),
  getHltbPendingMatches: () =>
    call<HltbPendingMatch[]>("get_hltb_pending_matches"),
  searchHltbCandidates: (query: string) =>
    call<HltbCandidate[]>("search_hltb_candidates", { query }),
  setHltbMatchOverride: (
    gameId: string,
    aliasQuery: string,
    candidate: HltbCandidate,
  ) =>
    call<void>("set_hltb_match_override", {
      gameId,
      aliasQuery,
      candidate,
    }),
  ignoreHltbMatch: (gameId: string, aliasQuery: string) =>
    call<void>("ignore_hltb_match", { gameId, aliasQuery }),
  clearHltbMatchOverride: (gameId: string) =>
    call<void>("clear_hltb_match_override", { gameId }),
  syncHltbLibrary: (onlyMissing: boolean) =>
    call<HltbSyncStatus>("sync_hltb_library", { onlyMissing }),
  cancelHltbSync: () => call<HltbSyncStatus>("cancel_hltb_sync"),
  getSteamGridDbConfiguration: () =>
    call<SteamGridDbConfigurationStatus>("get_steamgriddb_configuration"),
  saveSteamGridDbApiKey: (apiKey: string) =>
    call<SteamGridDbConfigurationStatus>("save_steamgriddb_api_key", {
      apiKey,
    }),
  deleteSteamGridDbApiKey: () =>
    call<SteamGridDbConfigurationStatus>("delete_steamgriddb_api_key"),
  getRapidApiReviewsConfiguration: () =>
    call<RapidApiReviewsConfigurationStatus>(
      "get_rapidapi_reviews_configuration",
    ),
  saveRapidApiReviewsApiKey: (apiKey: string) =>
    call<RapidApiReviewsConfigurationStatus>("save_rapidapi_reviews_api_key", {
      apiKey,
    }),
  deleteRapidApiReviewsApiKey: () =>
    call<RapidApiReviewsConfigurationStatus>("delete_rapidapi_reviews_api_key"),
  refreshSteamGameMetadata: (gameId: string) =>
    call<number>("refresh_steam_game_metadata", { gameId }),
  setGameFavorite: (gameId: string, favorite: boolean) =>
    call<boolean>("set_game_favorite", { gameId, favorite }),
  refreshSteamGameMetrics: (gameId: string) =>
    call<SteamGameMetrics>("refresh_steam_game_metrics", { gameId }),
  refreshSteamGameAchievements: (gameId: string) =>
    call<SteamGameMetrics>("refresh_steam_game_achievements", { gameId }),
  downloadSteamGameMedia: (gameId: string) =>
    call<number>("download_steam_game_media", { gameId }),
  cancelSteamImageSync: () =>
    call<SteamImageSyncStatus>("cancel_steam_image_sync"),
  getStorageStatus: () => call<StorageStatus>("get_storage_status"),
  migrateStorage: (targetMode: "appData" | "portable", deleteSource: boolean) =>
    call<StorageMigrationResult>("migrate_storage", {
      targetMode,
      deleteSource,
    }),
};
