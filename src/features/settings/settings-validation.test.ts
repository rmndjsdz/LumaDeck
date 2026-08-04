import { describe, expect, it } from "vitest";
import { validateSteamApiKey, validateSteamId64 } from "./settings-validation";

describe("Steam settings validation", () => {
  it("normalizes and validates SteamID64", () => {
    expect(validateSteamId64(" 76561198012345678 ")).toEqual({
      value: "76561198012345678",
    });
    expect(validateSteamId64("7656119801234567").error).toContain("17");
    expect(validateSteamId64("7656119801234567A").error).toContain("dígitos");
  });

  it("rejects empty and malformed API keys without exposing a value", () => {
    expect(validateSteamApiKey("").error).toContain("API Key");
    expect(validateSteamApiKey("invalid key with spaces").error).toContain(
      "caracteres",
    );
    expect(validateSteamApiKey("ABCDEFGHIJKLMNOP")).toEqual({
      value: "ABCDEFGHIJKLMNOP",
    });
  });
});
