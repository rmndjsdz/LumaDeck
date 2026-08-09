import { invoke } from "@tauri-apps/api/core";

export interface DisplayMode {
  displayId: string;
  deviceName: string;
  width: number;
  height: number;
  refreshRate: number;
}

export type DisplayResolutionMode = "SYSTEM" | "CUSTOM";
export type DisplayRefreshRateMode = "SYSTEM" | "CUSTOM";
export type DisplayHdrMode = "SYSTEM" | "OFF" | "ON" | "AUTO";
export type RtxHdrPreset = "NATURAL" | "VIBRANT";

export interface DisplayProfile {
  gameId: string;
  enabled: boolean;
  displayId: string | null;
  deviceName: string | null;
  width: number | null;
  height: number | null;
  refreshRate: number | null;
  restoreOnExit: boolean;
  updatedAt: string | null;
  resolutionMode: DisplayResolutionMode;
  refreshRateMode: DisplayRefreshRateMode;
  hdrMode: DisplayHdrMode;
  rtxHdrPreset: RtxHdrPreset | null;
  rtxHdrPeakNits: number;
}

const isDesktopRuntime = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const autoProfile = (gameId: string): DisplayProfile => ({
  gameId,
  enabled: false,
  displayId: null,
  deviceName: null,
  width: null,
  height: null,
  refreshRate: null,
  restoreOnExit: true,
  updatedAt: null,
  resolutionMode: "SYSTEM",
  refreshRateMode: "SYSTEM",
  hdrMode: "SYSTEM",
  rtxHdrPreset: null,
  rtxHdrPeakNits: 800,
});

export const displayProfileService = {
  getProfile(gameId: string): Promise<DisplayProfile> {
    if (!isDesktopRuntime()) return Promise.resolve(autoProfile(gameId));
    return invoke<DisplayProfile>("get_display_profile", { gameId });
  },

  getModes(): Promise<DisplayMode[]> {
    if (!isDesktopRuntime()) return Promise.resolve([]);
    return invoke<DisplayMode[]>("get_display_modes");
  },

  getCurrentMode(): Promise<DisplayMode> {
    if (!isDesktopRuntime()) {
      return Promise.reject(new Error("DISPLAY_WINDOWS_ONLY"));
    }
    return invoke<DisplayMode>("get_current_display_mode");
  },

  saveProfile(profile: DisplayProfile): Promise<DisplayProfile> {
    if (!isDesktopRuntime()) return Promise.resolve(profile);
    return invoke<DisplayProfile>("set_display_profile", { profile });
  },

  resetProfile(gameId: string): Promise<void> {
    if (!isDesktopRuntime()) return Promise.resolve();
    return invoke<void>("reset_display_profile", { gameId });
  },
};

export function displayProfileErrorMessage(error: unknown): string {
  const code = error instanceof Error ? error.message : String(error);
  if (code.includes("DISPLAY_MODE_UNAVAILABLE")) {
    return "La resolución o frecuencia ya no está disponible en esta pantalla.";
  }
  if (code.includes("DISPLAY_PROFILE_TARGET_REQUIRED")) {
    return "Selecciona una pantalla concreta para aplicar este perfil.";
  }
  if (code.includes("DISPLAY_PROFILE_OTHER_SESSION_ACTIVE")) {
    return "Ya existe otra sesión usando un perfil de pantalla.";
  }
  if (code.includes("DISPLAY_MODES_UNAVAILABLE")) {
    return "No se pudieron consultar los modos de pantalla disponibles.";
  }
  if (code.includes("DISPLAY_WINDOWS_ONLY")) {
    return "Los perfiles de pantalla están disponibles en la aplicación de Windows.";
  }
  return "No se pudo guardar el perfil de pantalla.";
}

export function formatDisplayResolution(
  width: number | null,
  height: number | null,
): string {
  return width && height ? `${width} × ${height}` : "Auto";
}

export function formatDisplayRefreshRate(refreshRate: number | null): string {
  return refreshRate ? `${refreshRate} Hz` : "Auto";
}
