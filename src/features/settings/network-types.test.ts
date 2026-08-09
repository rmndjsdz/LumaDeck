import { describe, expect, it } from "vitest";
import {
  classifyAdapter,
  networkErrorMessage,
  normalizeWifiNetworks,
  selectActiveConnection,
  type NetworkAdapter,
} from "./network-types";

const adapter = (patch: Partial<NetworkAdapter>): NetworkAdapter => ({
  id: "adapter",
  name: "Adapter",
  type: "other",
  state: "disconnected",
  connectionActive: false,
  dns: [],
  ...patch,
});

describe("network domain", () => {
  it("classifies common Windows adapter names", () => {
    expect(classifyAdapter("Wi-Fi", "Intel Wireless-AC")).toBe("wifi");
    expect(classifyAdapter("Ethernet 2", "USB Gigabit Ethernet")).toBe(
      "ethernet",
    );
    expect(classifyAdapter("vEthernet (Default Switch)")).toBe("other");
  });

  it("prefers a connected Ethernet adapter, then Wi-Fi", () => {
    const ethernet = adapter({
      id: "eth",
      type: "ethernet",
      connectionActive: true,
    });
    const wifi = adapter({ id: "wifi", type: "wifi", connectionActive: true });
    expect(selectActiveConnection([wifi, ethernet])?.id).toBe("eth");
  });

  it("deduplicates and orders Wi-Fi networks for distance navigation", () => {
    const networks = normalizeWifiNetworks([
      {
        ssid: " Cafe ",
        signalQuality: 40,
        security: "secured",
        connected: false,
        known: false,
        interfaceId: "wifi",
      },
      {
        ssid: "Cafe",
        signalQuality: 70,
        security: "secured",
        connected: true,
        known: true,
        interfaceId: "wifi",
      },
      {
        ssid: "Home",
        signalQuality: 50,
        security: "secured",
        connected: false,
        known: true,
        interfaceId: "wifi",
      },
    ]);
    expect(networks.map((network) => network.ssid)).toEqual(["Cafe", "Home"]);
    expect(networks[0]?.connected).toBe(true);
  });

  it("translates system failures into safe user-facing messages", () => {
    expect(networkErrorMessage("WRONG_PASSWORD")).toBe(
      "La contraseña parece incorrecta.",
    );
    expect(networkErrorMessage("NETWORK_NOT_FOUND")).toBe(
      "La red ya no está disponible.",
    );
    expect(networkErrorMessage("unexpected native detail")).toBe(
      "No se pudo completar la operación de red.",
    );
  });
});
