import { invoke } from "@tauri-apps/api/core";

export interface FrameGenerationProfile {
  gameId: string;
  provider: "lossless-scaling";
  enabled: boolean;
  mode: "FIXED";
  multiplier: 2 | 3 | 4;
  autoScale: boolean;
  autoScaleDelay: number;
  targetExecutable: string | null;
  updatedAt: string | null;
  restartRequired: boolean;
}

export interface LosslessScalingStatus {
  status: "Ready" | "NotInstalled" | "ConfigurationInvalid" | string;
  version: string;
  installationPath: string | null;
  settingsPath: string;
  settingsStatus: "valid" | "invalid" | "missing" | string;
  applicationRunning: boolean;
  restartRequired: boolean;
}

const isDesktopRuntime = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const fallbackProfile = (gameId: string): FrameGenerationProfile => ({
  gameId,
  provider: "lossless-scaling",
  enabled: false,
  mode: "FIXED",
  multiplier: 2,
  autoScale: true,
  autoScaleDelay: 0,
  targetExecutable: null,
  updatedAt: null,
  restartRequired: false,
});

export const frameGenerationService = {
  getProfile(gameId: string): Promise<FrameGenerationProfile> {
    if (!isDesktopRuntime()) return Promise.resolve(fallbackProfile(gameId));
    return invoke<FrameGenerationProfile>("get_frame_generation_profile", {
      gameId,
    });
  },

  saveProfile(
    profile: FrameGenerationProfile,
  ): Promise<FrameGenerationProfile> {
    if (!isDesktopRuntime()) return Promise.resolve(profile);
    return invoke<FrameGenerationProfile>("set_frame_generation_profile", {
      profile,
    });
  },

  getStatus(): Promise<LosslessScalingStatus> {
    if (!isDesktopRuntime()) {
      return Promise.resolve({
        status: "NotInstalled",
        version: "Unknown",
        installationPath: null,
        settingsPath: "",
        settingsStatus: "missing",
        applicationRunning: false,
        restartRequired: false,
      });
    }
    return invoke<LosslessScalingStatus>("get_lossless_scaling_status");
  },

  openApplication(): Promise<void> {
    return invoke<void>("open_lossless_scaling");
  },

  restoreBackup(): Promise<void> {
    return invoke<void>("restore_lossless_scaling_backup");
  },

  restartApplication(): Promise<void> {
    return invoke<void>("restart_lossless_scaling");
  },
};

export function frameGenerationLabel(
  profile: FrameGenerationProfile | null,
): string {
  if (!profile?.enabled) return "Off";
  return `LSFG ${profile.multiplier}x`;
}

export function frameGenerationErrorMessage(error: unknown): string {
  const code = error instanceof Error ? error.message : String(error);
  if (code.includes("LOSSLESS_SCALING_NOT_INSTALLED")) {
    return "Lossless Scaling no está instalado o no se encontró su configuración.";
  }
  if (code.includes("LOSSLESS_SCALING_SETTINGS_INVALID")) {
    return "Settings.xml de Lossless Scaling no es válido.";
  }
  if (code.includes("LOSSLESS_SCALING_DEFAULT_PROFILE_MISSING")) {
    return "Lossless Scaling no contiene el perfil Default requerido.";
  }
  if (code.includes("FRAME_GENERATION_PROFILE_INVALID")) {
    return "La configuración de Frame Generation no es válida.";
  }
  return "No se pudo guardar Frame Generation.";
}
