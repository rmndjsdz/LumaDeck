export type BluetoothAdapterStatus =
  "enabled" | "disabled" | "not-available" | "error";

export type BluetoothDeviceCategory =
  | "gamepad"
  | "headphones"
  | "headset"
  | "speaker"
  | "keyboard"
  | "mouse"
  | "phone"
  | "computer"
  | "other";

export type BluetoothPairingState =
  "paired" | "unpaired" | "pairing" | "unknown";

export type BluetoothConnectionState = "connected" | "disconnected" | "unknown";

export interface BluetoothAdapter {
  id: string;
  name: string;
  enabled: boolean;
  available: boolean;
  discoverable: boolean;
  hardwarePresent: boolean;
  status: BluetoothAdapterStatus;
  error?: string;
}

export interface BluetoothDevice {
  id: string;
  name: string;
  paired: boolean;
  connected: boolean;
  connectable: boolean;
  signalStrength?: number;
  deviceClass: BluetoothDeviceCategory;
  batteryLevel?: number;
  lastSeen?: string;
  pairingState: BluetoothPairingState;
  connectionState: BluetoothConnectionState;
}

export interface BluetoothSnapshot {
  adapters: BluetoothAdapter[];
  devices: BluetoothDevice[];
  discoveryActive: boolean;
  available: boolean;
  enabled: boolean;
}

export function bluetoothDeviceFocusId(deviceId: string): string {
  return `bluetooth-device-${encodeURIComponent(deviceId)}`;
}

export function normalizeBluetoothDevices(
  devices: readonly BluetoothDevice[],
): BluetoothDevice[] {
  const byIdentity = new Map<string, BluetoothDevice>();
  for (const candidate of devices) {
    const name = candidate.name.trim() || "Dispositivo Bluetooth";
    const normalized = { ...candidate, name };
    const identity = candidate.id.trim() || `name:${name.toLocaleLowerCase()}`;
    const current = byIdentity.get(identity);
    if (
      !current ||
      bluetoothDevicePriority(normalized) > bluetoothDevicePriority(current)
    ) {
      byIdentity.set(identity, normalized);
    }
  }

  return [...byIdentity.values()].sort((left, right) => {
    if (left.connected !== right.connected) return left.connected ? -1 : 1;
    if (left.paired !== right.paired) return left.paired ? -1 : 1;
    if (left.deviceClass !== right.deviceClass) {
      return categoryRank(left.deviceClass) - categoryRank(right.deviceClass);
    }
    const leftSignal = left.signalStrength ?? Number.NEGATIVE_INFINITY;
    const rightSignal = right.signalStrength ?? Number.NEGATIVE_INFINITY;
    if (Math.abs(leftSignal - rightSignal) >= 8)
      return rightSignal - leftSignal;
    return left.name.localeCompare(right.name, "es", { sensitivity: "base" });
  });
}

export function bluetoothErrorMessage(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  const normalized = raw.toUpperCase();
  if (normalized.includes("BLUETOOTH_UNAVAILABLE")) {
    return "Bluetooth no estÃ¡ disponible en este equipo.";
  }
  if (normalized.includes("BLUETOOTH_DISABLED")) {
    return "Bluetooth estÃ¡ desactivado.";
  }
  if (normalized.includes("BLUETOOTH_PAIRING_REJECTED")) {
    return "El emparejamiento fue rechazado.";
  }
  if (normalized.includes("BLUETOOTH_PAIRING_IN_PROGRESS")) {
    return "Windows todavía tiene otro emparejamiento en curso. Cancela la búsqueda, espera unos segundos y vuelve a iniciar una sola búsqueda.";
  }
  if (normalized.includes("BLUETOOTH_PAIRING_INTERACTION_REQUIRED")) {
    return "Windows requiere una confirmaciÃ³n o un PIN para completar este emparejamiento.";
  }
  if (normalized.includes("BLUETOOTH_PAIRING_CLEANUP_FAILED")) {
    return "Windows no confirmÃ³ la cancelaciÃ³n del emparejamiento; espera a que termine antes de intentarlo de nuevo.";
  }
  if (normalized.includes("BLUETOOTH_DISCOVERY_STOP_TIMEOUT")) {
    return "Windows no confirmÃ³ el cierre de la bÃºsqueda Bluetooth.";
  }
  if (normalized.includes("BLUETOOTH_PAIRING_RECOVERY_FAILED")) {
    return "No se pudo restablecer el servicio de emparejamiento de Windows. Intenta nuevamente o reinicia el equipo.";
  }
  if (normalized.includes("BLUETOOTH_DEVICE_GONE")) {
    return "El dispositivo dejÃ³ de estar disponible.";
  }
  if (normalized.includes("BLUETOOTH_DEVICE_NOT_READY")) {
    return "El DualSense no está en modo de emparejamiento o Windows conserva una entrada antigua. Apágalo, mantén Create + PS hasta que parpadee y vuelve a buscar.";
  }
  if (normalized.includes("BLUETOOTH_FORGET_FAILED")) {
    return "No se pudo olvidar el dispositivo.";
  }
  if (normalized.includes("BLUETOOTH_RADIO_ACCESS_DENIED")) {
    return "Windows no permitiÃ³ cambiar el estado de Bluetooth.";
  }
  if (normalized.includes("BLUETOOTH_PAIRING_FAILED")) {
    return "No se pudo emparejar el dispositivo.";
  }
  if (normalized.includes("BLUETOOTH_PAIRING_TIMEOUT")) {
    return "El emparejamiento tardó demasiado. Comprueba que el DualSense siga en modo pairing e inténtalo de nuevo.";
  }
  return "No se pudo completar la operaciÃ³n de Bluetooth.";
}

function bluetoothDevicePriority(device: BluetoothDevice): number {
  return (
    Number(device.connected) * 4 +
    Number(device.paired) * 2 +
    Number(device.connectable)
  );
}

function categoryRank(category: BluetoothDeviceCategory): number {
  return {
    gamepad: 0,
    headphones: 1,
    headset: 2,
    speaker: 3,
    keyboard: 4,
    mouse: 5,
    phone: 6,
    computer: 7,
    other: 8,
  }[category];
}
