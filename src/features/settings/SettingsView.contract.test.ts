import { describe, expect, it } from "vitest";
import { PROVIDERS, providerAvailability } from "./settings-provider-catalog";

describe("LaunchBox Settings provider", () => {
  it("is visible as a configurable local metadata provider", () => {
    expect(PROVIDERS).toContainEqual([
      "launchbox",
      "LaunchBox",
      "Metadatos y capturas para juegos emulados",
      "L",
    ]);
    expect(providerAvailability("launchbox")).toBe("available");
  });
});
