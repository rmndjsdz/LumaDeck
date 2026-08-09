import { describe, expect, it } from "vitest";
import {
  bluetoothDeviceFocusId,
  bluetoothErrorMessage,
  normalizeBluetoothDevices,
  type BluetoothDevice,
} from "./bluetooth-types";

const device = (overrides: Partial<BluetoothDevice>): BluetoothDevice => ({
  id: "device-1",
  name: "Controller",
  paired: false,
  connected: false,
  connectable: true,
  deviceClass: "gamepad",
  pairingState: "unpaired",
  connectionState: "disconnected",
  ...overrides,
});

describe("Bluetooth domain helpers", () => {
  it("keeps stable identity when discovery adds entries", () => {
    const first = device({ id: "a", name: "A" });
    const second = device({ id: "b", name: "B" });
    expect(bluetoothDeviceFocusId(first.id)).toBe("bluetooth-device-a");
    expect(
      normalizeBluetoothDevices([first, second]).map((item) => item.id),
    ).toEqual(["a", "b"]);
    expect(
      normalizeBluetoothDevices([device({ id: "new" }), first, second]).find(
        (item) => item.id === "a",
      ),
    ).toBeDefined();
  });

  it("deduplicates by identity and keeps the richer observation", () => {
    const normalized = normalizeBluetoothDevices([
      device({ id: "same", paired: false }),
      device({ id: "same", paired: true, connected: true }),
    ]);
    expect(normalized).toHaveLength(1);
    expect(normalized[0]?.connected).toBe(true);
  });

  it("orders connected and paired devices before new discovery results", () => {
    const normalized = normalizeBluetoothDevices([
      device({ id: "new", name: "New" }),
      device({
        id: "paired",
        name: "Paired",
        paired: true,
        pairingState: "paired",
      }),
      device({
        id: "connected",
        name: "Connected",
        paired: true,
        connected: true,
        connectionState: "connected",
      }),
    ]);
    expect(normalized.map((item) => item.id)).toEqual([
      "connected",
      "paired",
      "new",
    ]);
  });

  it("translates backend errors without exposing HRESULTs", () => {
    expect(bluetoothErrorMessage("BLUETOOTH_PAIRING_REJECTED")).toBe(
      "El emparejamiento fue rechazado.",
    );
    expect(bluetoothErrorMessage("0x80070490")).not.toContain("0x80070490");
  });

  it("explains native Windows pairing recovery failures", () => {
    expect(
      bluetoothErrorMessage("BLUETOOTH_PAIRING_RECOVERY_FAILED"),
    ).toContain("restablecer el servicio");
  });
});
