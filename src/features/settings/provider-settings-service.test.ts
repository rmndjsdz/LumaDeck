import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
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
});
