import { invoke } from "@tauri-apps/api/core";
import type { NetworkSnapshot } from "./network-types";

export interface NetworkService {
  getSnapshot(): Promise<NetworkSnapshot>;
  scanWifi(): Promise<NetworkSnapshot>;
  setAdapterEnabled(
    adapterId: string,
    enabled: boolean,
  ): Promise<NetworkSnapshot>;
  setWifiEnabled(adapterId: string, enabled: boolean): Promise<NetworkSnapshot>;
  connectWifi(
    adapterId: string,
    ssid: string,
    password?: string,
  ): Promise<NetworkSnapshot>;
  disconnectWifi(adapterId: string): Promise<NetworkSnapshot>;
  forgetWifi(adapterId: string, ssid: string): Promise<NetworkSnapshot>;
}

const networkService: NetworkService = {
  getSnapshot: () => invoke<NetworkSnapshot>("get_network_state"),
  scanWifi: () => invoke<NetworkSnapshot>("scan_wifi_networks"),
  setAdapterEnabled: (adapterId, enabled) =>
    invoke<NetworkSnapshot>("set_network_adapter_enabled", {
      adapterId,
      enabled,
    }),
  setWifiEnabled: (adapterId, enabled) =>
    invoke<NetworkSnapshot>("set_wifi_enabled", { adapterId, enabled }),
  connectWifi: (adapterId, ssid, password) =>
    invoke<NetworkSnapshot>("connect_wifi", { adapterId, ssid, password }),
  disconnectWifi: (adapterId) =>
    invoke<NetworkSnapshot>("disconnect_wifi", { adapterId }),
  forgetWifi: (adapterId, ssid) =>
    invoke<NetworkSnapshot>("forget_wifi", { adapterId, ssid }),
};

export { networkService };
