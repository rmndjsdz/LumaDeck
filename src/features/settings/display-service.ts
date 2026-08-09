import { invoke } from "@tauri-apps/api/core";
import { hasReachedHdrTarget } from "./display-types";
import type {
  DisplayInfo,
  DisplaySnapshot,
  DisplayMode,
  DisplayModeChange,
  DisplayScale,
  HdrSnapshot,
  HdrState,
} from "./display-types";

export interface DisplayService {
  getDisplays(): Promise<DisplayInfo[]>;
  getCurrentDisplayMode(displayId?: string): Promise<DisplayMode>;
  getSupportedDisplayModes(displayId: string): Promise<DisplayMode[]>;
  beginDisplayModeChange(mode: DisplayMode): Promise<DisplayModeChange>;
  confirmDisplayModeChange(): Promise<DisplayMode>;
  rollbackDisplayModeChange(): Promise<DisplayMode>;
  getDisplayScale(displayId: string): Promise<DisplayScale>;
  setDisplayScale(displayId: string, scale: number): Promise<DisplayScale>;
  getHdrState(displayId: string): Promise<HdrState>;
  setHdrEnabled(displayId: string, enabled: boolean): Promise<HdrState>;
  captureHdrState(displayId: string): Promise<HdrSnapshot>;
  restoreHdrState(snapshot: HdrSnapshot): Promise<HdrState>;
  captureDisplaySnapshot(displayId: string): Promise<DisplaySnapshot>;
  restoreDisplaySnapshot(snapshot: DisplaySnapshot): Promise<DisplaySnapshot>;
}

const isDesktopRuntime = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const unavailable = <T>(): Promise<T> =>
  Promise.reject(new Error("DISPLAY_WINDOWS_ONLY"));

const displayService: DisplayService = {
  getDisplays: () =>
    isDesktopRuntime()
      ? invoke<DisplayInfo[]>("get_displays")
      : unavailable<DisplayInfo[]>(),
  getCurrentDisplayMode: (displayId) =>
    isDesktopRuntime()
      ? invoke<DisplayMode>("get_current_display_mode", { displayId })
      : unavailable<DisplayMode>(),
  getSupportedDisplayModes: (displayId) =>
    isDesktopRuntime()
      ? invoke<DisplayMode[]>("get_display_modes_for_display", { displayId })
      : unavailable<DisplayMode[]>(),
  beginDisplayModeChange: (mode) =>
    isDesktopRuntime()
      ? invoke<DisplayModeChange>("begin_display_mode_change", {
          request: mode,
        })
      : unavailable<DisplayModeChange>(),
  confirmDisplayModeChange: () =>
    isDesktopRuntime()
      ? invoke<DisplayMode>("confirm_display_mode_change")
      : unavailable<DisplayMode>(),
  rollbackDisplayModeChange: () =>
    isDesktopRuntime()
      ? invoke<DisplayMode>("rollback_display_mode_change")
      : unavailable<DisplayMode>(),
  getDisplayScale: (displayId) =>
    isDesktopRuntime()
      ? invoke<DisplayScale>("get_display_scale", { displayId })
      : unavailable<DisplayScale>(),
  setDisplayScale: (displayId, scale) =>
    isDesktopRuntime()
      ? invoke<DisplayScale>("set_display_scale", { displayId, scale })
      : unavailable<DisplayScale>(),
  getHdrState: (displayId) =>
    isDesktopRuntime()
      ? invoke<HdrState>("get_hdr_state", { displayId })
      : unavailable<HdrState>(),
  setHdrEnabled: async (displayId, enabled) => {
    if (!isDesktopRuntime()) return unavailable<HdrState>();
    const state = await invoke<HdrState>("set_hdr_enabled", {
      displayId,
      enabled,
    });
    if (!hasReachedHdrTarget(state, enabled)) {
      throw new Error("DISPLAY_HDR_VERIFY_FAILED");
    }
    return state;
  },
  captureHdrState: (displayId) =>
    isDesktopRuntime()
      ? invoke<HdrSnapshot>("capture_hdr_state", { displayId })
      : unavailable<HdrSnapshot>(),
  restoreHdrState: (snapshot) =>
    isDesktopRuntime()
      ? invoke<HdrState>("restore_hdr_state", { snapshot })
      : unavailable<HdrState>(),
  captureDisplaySnapshot: (displayId) =>
    isDesktopRuntime()
      ? invoke<DisplaySnapshot>("capture_display_snapshot", { displayId })
      : unavailable<DisplaySnapshot>(),
  restoreDisplaySnapshot: (snapshot) =>
    isDesktopRuntime()
      ? invoke<DisplaySnapshot>("restore_display_snapshot", { snapshot })
      : unavailable<DisplaySnapshot>(),
};

export { displayService };
