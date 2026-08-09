import { invoke } from "@tauri-apps/api/core";
import { displayService } from "../settings/display-service";
import type { DisplayInfo } from "../settings/display-types";
import { displayProfileService } from "../launcher/display-profile-service";
import type { ResolvedGameCapabilities } from "../game-capabilities/game-capabilities-types";
import {
  unknownDisplay,
  unknownHardware,
  type DisplayCapabilities,
  type GraphicsProfileInput,
  type HardwareCapabilities,
  type RecommendedGraphicsProfile,
} from "./graphics-profile-types";

const isDesktopRuntime = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function readDisplayCapabilities(
  gameId?: string,
): Promise<DisplayCapabilities> {
  if (!isDesktopRuntime()) return unknownDisplay;
  const displays = await displayService.getDisplays();
  const profile = gameId
    ? await displayProfileService.getProfile(gameId).catch(() => null)
    : null;
  const selected = selectDisplay(displays, profile?.displayId ?? null);
  if (!selected) return unknownDisplay;
  const modes = await displayService
    .getSupportedDisplayModes(selected.id)
    .catch(() => []);
  const current = selected.currentMode;
  const resolutions = uniqueResolutions(modes);
  return {
    displayId: selected.id,
    currentResolution: current
      ? { width: current.width, height: current.height }
      : null,
    supportedResolutions: resolutions,
    currentRefreshRate: current?.refreshRate ?? null,
    supportedRefreshRates: [...new Set(modes.map((mode) => mode.refreshRate))],
    hdrSupported: selected.hdrSupported,
    hdrEnabled: selected.hdrEnabled,
  };
}

export const graphicsProfileService = {
  resolve(
    gameId: string,
    gameCapabilities: ResolvedGameCapabilities,
    display: DisplayCapabilities,
    hardware: HardwareCapabilities = unknownHardware,
  ): Promise<RecommendedGraphicsProfile> {
    const input: GraphicsProfileInput = {
      gameId,
      gameCapabilities,
      hardware,
      display,
    };
    if (!isDesktopRuntime()) {
      return Promise.reject(
        new Error("GRAPHICS_PROFILE_DESKTOP_RUNTIME_UNAVAILABLE"),
      );
    }
    return invoke<RecommendedGraphicsProfile>("resolve_graphics_profile", {
      input,
    });
  },
};

function selectDisplay(
  displays: readonly DisplayInfo[],
  selectedDisplayId: string | null,
): DisplayInfo | null {
  return (
    displays.find(
      (display) => display.id === selectedDisplayId && display.connected,
    ) ??
    displays.find((display) => display.primary && display.connected) ??
    displays.find((display) => display.connected) ??
    null
  );
}

function uniqueResolutions(
  modes: readonly { width: number; height: number }[],
): { width: number; height: number }[] {
  const seen = new Set<string>();
  return modes.filter((mode) => {
    const key = `${mode.width}x${mode.height}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
