import { describe, expect, it } from "vitest";
import {
  canChangeHdr,
  displayErrorMessage,
  hasReachedHdrTarget,
  hdrStatusLabel,
  modesForResolution,
  selectCompatibleMode,
  uniqueModes,
} from "./display-types";
import type { DisplayMode } from "./display-types";

const modes: DisplayMode[] = [
  {
    displayId: "DISPLAY1",
    deviceName: "DISPLAY1",
    width: 3840,
    height: 2160,
    refreshRate: 60,
  },
  {
    displayId: "DISPLAY1",
    deviceName: "DISPLAY1",
    width: 2560,
    height: 1440,
    refreshRate: 60,
  },
  {
    displayId: "DISPLAY1",
    deviceName: "DISPLAY1",
    width: 2560,
    height: 1440,
    refreshRate: 120,
  },
  {
    displayId: "DISPLAY1",
    deviceName: "DISPLAY1",
    width: 2560,
    height: 1440,
    refreshRate: 120,
  },
];

describe("display mode domain", () => {
  it("removes duplicate complete modes", () => {
    expect(uniqueModes(modes)).toHaveLength(3);
  });

  it("keeps refresh rates tied to a resolution", () => {
    expect(
      modesForResolution(modes, 3840, 2160).map((mode) => mode.refreshRate),
    ).toEqual([60]);
    expect(
      modesForResolution(modes, 2560, 1440).map((mode) => mode.refreshRate),
    ).toEqual([120, 120, 60]);
  });

  it("prefers the current refresh rate and otherwise chooses a valid mode", () => {
    expect(selectCompatibleMode(modes, 3840, 2160, 120)?.refreshRate).toBe(60);
    expect(selectCompatibleMode(modes, 2560, 1440, 120)?.refreshRate).toBe(120);
  });
});

describe("HDR display domain", () => {
  const sdr = {
    displayId: "DISPLAY1",
    supported: false,
    enabled: false,
    status: "unsupported" as const,
  };
  const hdrOff = {
    displayId: "DISPLAY2",
    supported: true,
    enabled: false,
    status: "supported" as const,
  };
  const hdrOn = { ...hdrOff, displayId: "DISPLAY3", enabled: true };
  const unknown = {
    displayId: "DISPLAY3",
    supported: null,
    enabled: null,
    status: "unknown" as const,
  };

  it("distinguishes SDR, HDR disabled, HDR enabled, and unknown state", () => {
    expect(canChangeHdr(sdr)).toBe(false);
    expect(canChangeHdr(hdrOff)).toBe(true);
    expect(hdrStatusLabel(hdrOff)).toBe("Desactivado");
    expect(hdrStatusLabel(hdrOn)).toBe("Activado");
    expect(hdrStatusLabel(unknown)).toBe("No disponible");
  });

  it("reconciles both directions only after Windows reports the target", () => {
    expect(hasReachedHdrTarget(hdrOn, true)).toBe(true);
    expect(hasReachedHdrTarget(hdrOn, false)).toBe(false);
    expect(hasReachedHdrTarget(hdrOff, false)).toBe(true);
  });

  it("keeps monitor state isolated for multi-monitor setups", () => {
    const displays = [hdrOff, hdrOn];
    expect(displays.map((display) => display.enabled)).toEqual([false, true]);
    expect(displays[0].displayId).not.toBe(displays[1].displayId);
  });

  it("maps native rejection and hotplug errors to user-safe messages", () => {
    expect(displayErrorMessage(new Error("DISPLAY_HDR_APPLY_FAILED:5"))).toBe(
      "Windows no pudo actualizar HDR.",
    );
    expect(displayErrorMessage(new Error("DISPLAY_HDR_VERIFY_FAILED"))).toBe(
      "Windows no pudo confirmar el cambio de HDR.",
    );
    expect(displayErrorMessage(new Error("DISPLAY_NOT_FOUND"))).toBe(
      "La pantalla ya no está disponible.",
    );
  });

  it("preserves an HDR-only snapshot independently of display mode", () => {
    const snapshot = { displayId: "DISPLAY2", enabled: true };
    expect(snapshot).toEqual({ displayId: "DISPLAY2", enabled: true });
    expect(snapshot).not.toHaveProperty("width");
    expect(snapshot).not.toHaveProperty("refreshRate");
  });
});
