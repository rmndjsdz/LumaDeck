export interface DisplayMode {
  displayId: string;
  deviceName: string;
  width: number;
  height: number;
  refreshRate: number;
}

export interface DisplayScale {
  current: number | null;
  recommended: number | null;
  supported: number[];
  available: boolean;
  canChange: boolean;
}

export type HdrStatus = "supported" | "unsupported" | "unknown";

export interface HdrState {
  displayId: string;
  supported: boolean | null;
  enabled: boolean | null;
  status: HdrStatus;
}

export interface HdrSnapshot {
  displayId: string;
  enabled: boolean;
}

export interface DisplaySnapshot {
  displayId: string;
  mode: DisplayMode | null;
  scale: DisplayScale;
  hdr: HdrSnapshot;
}

export interface DisplayInfo {
  id: string;
  name: string;
  friendlyName: string | null;
  primary: boolean;
  connected: boolean;
  currentMode: DisplayMode | null;
  scale: DisplayScale;
  hdrSupported: boolean | null;
  hdrEnabled: boolean | null;
  hdrStatus: HdrStatus;
}

export interface DisplayModeChange {
  previousMode: DisplayMode;
  appliedMode: DisplayMode;
  expiresAtMs: number;
}

export function sameDisplayMode(
  left: DisplayMode | null | undefined,
  right: DisplayMode | null | undefined,
): boolean {
  return Boolean(
    left &&
    right &&
    left.displayId === right.displayId &&
    left.width === right.width &&
    left.height === right.height &&
    left.refreshRate === right.refreshRate,
  );
}

export function uniqueModes(modes: readonly DisplayMode[]): DisplayMode[] {
  const seen = new Set<string>();
  return modes.filter((mode) => {
    const key = `${mode.displayId}:${mode.width}:${mode.height}:${mode.refreshRate}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function modesForResolution(
  modes: readonly DisplayMode[],
  width: number,
  height: number,
): DisplayMode[] {
  return modes
    .filter((mode) => mode.width === width && mode.height === height)
    .sort((left, right) => right.refreshRate - left.refreshRate);
}

export function selectCompatibleMode(
  modes: readonly DisplayMode[],
  width: number,
  height: number,
  preferredRefreshRate: number,
): DisplayMode | null {
  const compatible = modesForResolution(modes, width, height);
  return (
    compatible.find((mode) => mode.refreshRate === preferredRefreshRate) ??
    compatible[0] ??
    null
  );
}

export function canChangeHdr(state: HdrState | null | undefined): boolean {
  return state?.status === "supported" && state.supported === true;
}

export function hdrStatusLabel(state: HdrState | null | undefined): string {
  if (!state || state.status === "unknown") return "No disponible";
  if (state.status === "unsupported") return "No disponible";
  if (state.enabled === true) return "Activado";
  if (state.enabled === false) return "Desactivado";
  return "No disponible";
}

export function hasReachedHdrTarget(
  state: HdrState | null | undefined,
  enabled: boolean,
): boolean {
  return (
    state?.status === "supported" &&
    state.supported === true &&
    state.enabled === enabled
  );
}

export function displayErrorMessage(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  const code = raw.toUpperCase();
  if (code.includes("DISPLAY_MODE_UNAVAILABLE")) {
    return "Esta resolución o frecuencia ya no está disponible.";
  }
  if (code.includes("DISPLAY_MODE_TEST_REJECTED")) {
    return "Windows rechazó esta configuración.";
  }
  if (code.includes("DISPLAY_MODE_VERIFY_FAILED")) {
    return "Windows no aplicó el modo solicitado.";
  }
  if (code.includes("DISPLAY_SCALE_CHANGE_UNSUPPORTED")) {
    return "Windows no expone un cambio de escala seguro mediante una API soportada.";
  }
  if (code.includes("DISPLAY_SCALE_UNAVAILABLE")) {
    return "No se pudo consultar la escala de esta pantalla.";
  }
  if (code.includes("DISPLAY_HDR_UNSUPPORTED")) {
    return "Esta pantalla no admite HDR.";
  }
  if (
    code.includes("DISPLAY_HDR_UNAVAILABLE") ||
    code.includes("DISPLAY_HDR_STATE")
  ) {
    return "HDR ya no estÃ¡ disponible con la configuraciÃ³n actual.";
  }
  if (code.includes("DISPLAY_HDR_VERIFY_FAILED")) {
    return "Windows no pudo confirmar el cambio de HDR.";
  }
  if (code.includes("DISPLAY_HDR_APPLY_FAILED")) {
    return "Windows no pudo actualizar HDR.";
  }
  if (
    code.includes("DISPLAY_NOT_FOUND") ||
    code.includes("DISPLAY_CURRENT_MODE")
  ) {
    return "La pantalla ya no está disponible.";
  }
  if (code.includes("DISPLAY_WINDOWS_ONLY")) {
    return "La configuración de pantalla está disponible en Windows.";
  }
  return "No se pudo actualizar la configuración de pantalla.";
}
