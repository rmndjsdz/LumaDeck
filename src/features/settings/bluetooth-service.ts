import { invoke } from "@tauri-apps/api/core";
import type { BluetoothSnapshot } from "./bluetooth-types";

export interface BluetoothTelemetryEvent {
  timestamp: string;
  level: string;
  event: string;
  details: string;
}

export interface BluetoothService {
  getSnapshot(): Promise<BluetoothSnapshot>;
  setEnabled(enabled: boolean): Promise<BluetoothSnapshot>;
  startDiscovery(): Promise<BluetoothSnapshot>;
  stopDiscovery(): Promise<BluetoothSnapshot>;
  pairDevice(deviceId: string): Promise<BluetoothSnapshot>;
  unpairDevice(deviceId: string): Promise<BluetoothSnapshot>;
  getDiagnostics(): Promise<BluetoothTelemetryEvent[]>;
  recordClientDiagnostic(
    level: "info" | "warn" | "error",
    event: string,
    details: string,
  ): Promise<void>;
}

const bluetoothService: BluetoothService = {
  getSnapshot: () => invoke<BluetoothSnapshot>("get_bluetooth_state"),
  setEnabled: (enabled) =>
    invoke<BluetoothSnapshot>("set_bluetooth_enabled", { enabled }),
  startDiscovery: () => invoke<BluetoothSnapshot>("start_bluetooth_discovery"),
  stopDiscovery: () => invoke<BluetoothSnapshot>("stop_bluetooth_discovery"),
  pairDevice: (deviceId) =>
    invoke<BluetoothSnapshot>("pair_bluetooth_device", { deviceId }),
  unpairDevice: (deviceId) =>
    invoke<BluetoothSnapshot>("unpair_bluetooth_device", { deviceId }),
  getDiagnostics: () =>
    invoke<BluetoothTelemetryEvent[]>("get_bluetooth_diagnostics"),
  recordClientDiagnostic: (level, event, details) =>
    invoke("record_bluetooth_client_diagnostic", { level, event, details }),
};

export { bluetoothService };
