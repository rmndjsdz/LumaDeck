import { invoke } from "@tauri-apps/api/core";
import {
  unknownHardware,
  type HardwareCapabilities,
} from "./graphics-profile-types";

const isDesktopRuntime = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const hardwareCapabilitiesService = {
  get(): Promise<HardwareCapabilities> {
    if (!isDesktopRuntime()) return Promise.resolve(unknownHardware);
    return invoke<HardwareCapabilities>("get_hardware_capabilities");
  },

  refresh(): Promise<HardwareCapabilities> {
    if (!isDesktopRuntime()) return Promise.resolve(unknownHardware);
    return invoke<HardwareCapabilities>("refresh_hardware_capabilities");
  },
};

export function hardwareVendorLabel(
  vendor: HardwareCapabilities["vendor"],
): string {
  if (vendor === "NVIDIA") return "NVIDIA";
  if (vendor === "AMD") return "AMD";
  if (vendor === "INTEL") return "Intel";
  if (vendor === "OTHER") return "Otro";
  return "Desconocida";
}

export function formatVram(vramMb: number | null): string {
  if (vramMb === null) return "VRAM desconocida";
  return `${(vramMb / 1024).toFixed(vramMb >= 1024 ? 1 : 0)} GB VRAM`;
}
