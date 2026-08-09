import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  launchBoxErrorMessage,
  providerSettingsService,
  type SettingsSaveCorrelationId,
} from "./provider-settings-service";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);
const correlationId = "settings-save-contract" as SettingsSaveCorrelationId;

describe("Settings Tauri IPC contract", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({});
  });

  it("sends the SteamID-only save payload in camelCase", async () => {
    await providerSettingsService.saveSteamConfiguration(
      "76561198012345678",
      "",
      correlationId,
    );

    expect(invokeMock).toHaveBeenCalledWith(
      "save_steam_account_configuration",
      {
        steamId64: "76561198012345678",
        apiKey: "",
        correlationId,
      },
    );
    const payload = invokeMock.mock.calls[0]?.[1];
    expect(payload).not.toHaveProperty("steam_id64");
    expect(payload).not.toHaveProperty("api_key");
    expect(payload).not.toHaveProperty("correlation_id");
  });

  it("uses camelCase for every Settings and Steam command payload", async () => {
    await providerSettingsService.updateSteamId(
      "76561198012345678",
      correlationId,
    );
    await providerSettingsService.replaceSteamApiKey(
      "ABCDEFGHIJKLMNOP",
      correlationId,
    );
    await providerSettingsService.disconnect("steam-default");
    await providerSettingsService.getSteamConfiguration();
    await providerSettingsService.getDatabaseStatus();
    await providerSettingsService.getSteamProfile();

    expect(invokeMock.mock.calls).toEqual([
      ["update_steam_id", { steamId64: "76561198012345678", correlationId }],
      ["replace_steam_api_key", { apiKey: "ABCDEFGHIJKLMNOP", correlationId }],
      ["disconnect_provider_account", { accountId: "steam-default" }],
      ["get_provider_configuration", { providerId: "steam" }],
      ["get_database_status", undefined],
      ["get_steam_profile", undefined],
    ]);
    for (const [, payload] of invokeMock.mock.calls) {
      if (!payload || typeof payload !== "object") continue;
      expect(payload).not.toHaveProperty("steam_id64");
      expect(payload).not.toHaveProperty("api_key");
      expect(payload).not.toHaveProperty("correlation_id");
      expect(payload).not.toHaveProperty("provider_id");
      expect(payload).not.toHaveProperty("account_id");
    }
  });

  it("classifies an IPC argument rejection separately from a database error", async () => {
    invokeMock.mockRejectedValueOnce(
      "invalid args `steamId64` for command `save_steam_account_configuration`: command save_steam_account_configuration missing required key steamId64",
    );

    await expect(
      providerSettingsService.saveSteamConfiguration(
        "76561198012345678",
        "",
        correlationId,
      ),
    ).rejects.toMatchObject({ code: "IPC_INVALID_ARGUMENTS" });

    invokeMock.mockRejectedValueOnce("DATABASE_ERROR");
    await expect(
      providerSettingsService.saveSteamConfiguration(
        "76561198012345678",
        "",
        correlationId,
      ),
    ).rejects.toMatchObject({ code: "DATABASE_ERROR" });
  });

  it("uses one shared RapidAPI credential for OpenCritic and Metacritic", async () => {
    await providerSettingsService.getRapidApiReviewsConfiguration();
    await providerSettingsService.saveRapidApiReviewsApiKey("rapid-api-key");
    await providerSettingsService.deleteRapidApiReviewsApiKey();

    expect(invokeMock.mock.calls).toEqual([
      ["get_rapidapi_reviews_configuration", undefined],
      ["save_rapidapi_reviews_api_key", { apiKey: "rapid-api-key" }],
      ["delete_rapidapi_reviews_api_key", undefined],
    ]);
  });

  it("keeps AI credentials and model arguments in the Rust IPC boundary", async () => {
    await providerSettingsService.getAIConfiguration();
    await providerSettingsService.saveAIConfiguration(
      "openrouter",
      "google/gemini-2.5-flash",
      "sk-or-v1-test-key",
    );
    await providerSettingsService.testAIConnection(
      "openrouter",
      "google/gemini-2.5-flash",
      "",
    );

    expect(invokeMock.mock.calls).toEqual([
      ["get_ai_configuration", undefined],
      [
        "save_ai_configuration",
        {
          providerId: "openrouter",
          model: "google/gemini-2.5-flash",
          apiKey: "sk-or-v1-test-key",
        },
      ],
      [
        "test_ai_connection",
        {
          providerId: "openrouter",
          model: "google/gemini-2.5-flash",
          apiKey: "",
        },
      ],
    ]);
    for (const [, payload] of invokeMock.mock.calls) {
      if (!payload || typeof payload !== "object") continue;
      expect(payload).not.toHaveProperty("provider_id");
      expect(payload).not.toHaveProperty("api_key");
      expect(payload).toHaveProperty("model");
    }
  });

  it("uses the LaunchBox catalog and per-game refresh IPC commands", async () => {
    await providerSettingsService.getLaunchBoxCatalogStatus();
    await providerSettingsService.refreshLaunchBoxCatalog(true);
    await providerSettingsService.refreshGameMetadata("emulator-mario-kart");
    await providerSettingsService.downloadLaunchBoxScreenshots(
      "emulator-mario-kart",
    );

    expect(invokeMock.mock.calls).toEqual([
      ["get_launchbox_catalog_status", undefined],
      ["refresh_launchbox_catalog", { force: true }],
      ["refresh_game_metadata", { gameId: "emulator-mario-kart" }],
      ["download_launchbox_screenshots", { gameId: "emulator-mario-kart" }],
    ]);
  });

  it("maps known LaunchBox backend states to user-facing messages", () => {
    expect(launchBoxErrorMessage("LAUNCHBOX_CATALOG_NOT_READY")).toContain(
      "todavía se está preparando",
    );
    expect(launchBoxErrorMessage("LAUNCHBOX_DATABASE_LOCK")).toContain(
      "temporalmente ocupado",
    );
    expect(launchBoxErrorMessage("LAUNCHBOX_CATALOG_UNAVAILABLE")).toContain(
      "no está disponible",
    );
    expect(
      launchBoxErrorMessage("LAUNCHBOX_UPDATE_FAILED_WITH_FALLBACK"),
    ).toContain("versión anterior");
    expect(launchBoxErrorMessage("UNKNOWN_ERROR")).toBeNull();
  });
});
