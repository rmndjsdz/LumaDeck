export type NetworkAdapterType = "ethernet" | "wifi" | "other";

export type NetworkAdapterState =
  | "connected"
  | "connected-no-internet"
  | "disconnected"
  | "disabled"
  | "connecting";

export type InternetState =
  "connected" | "connected-no-internet" | "disconnected" | "connecting";

export interface NetworkAdapter {
  id: string;
  name: string;
  type: NetworkAdapterType;
  state: NetworkAdapterState;
  connectionActive: boolean;
  ipv4?: string;
  ipv6?: string;
  gateway?: string;
  dns: string[];
  mac?: string;
  linkSpeed?: string;
  wifiInterfaceId?: string;
  interfaceIndex?: number;
}

export interface WifiNetwork {
  ssid: string;
  signalQuality: number;
  security: "open" | "secured";
  connected: boolean;
  known: boolean;
  interfaceId: string;
}

export interface NetworkSnapshot {
  adapters: NetworkAdapter[];
  wifiNetworks: WifiNetwork[];
  internetState: InternetState;
  activeConnectionType: NetworkAdapterType | null;
  wifiEnabled: boolean;
}

export function classifyAdapter(
  name: string,
  description = "",
): NetworkAdapterType {
  const value = `${name} ${description}`.toLocaleLowerCase("en-US");
  if (/vethernet|virtual|hyper-v|loopback/.test(value)) return "other";
  if (/(wi-?fi|wireless|802\.11|wlan)/.test(value)) return "wifi";
  if (/(ethernet|gigabit|lan|local area)/.test(value)) return "ethernet";
  return "other";
}

export function selectActiveConnection(
  adapters: readonly NetworkAdapter[],
): NetworkAdapter | null {
  return (
    adapters.find(
      (adapter) => adapter.connectionActive && adapter.type === "ethernet",
    ) ??
    adapters.find(
      (adapter) => adapter.connectionActive && adapter.type === "wifi",
    ) ??
    adapters.find((adapter) => adapter.connectionActive) ??
    null
  );
}

export function normalizeWifiNetworks(
  networks: readonly WifiNetwork[],
): WifiNetwork[] {
  const bySsid = new Map<string, WifiNetwork>();
  for (const network of networks) {
    const ssid = network.ssid.trim();
    if (!ssid) continue;
    const normalized: WifiNetwork = { ...network, ssid };
    const current = bySsid.get(ssid);
    if (
      !current ||
      Number(normalized.connected) > Number(current.connected) ||
      Number(normalized.known) > Number(current.known) ||
      normalized.signalQuality > current.signalQuality
    ) {
      bySsid.set(ssid, normalized);
    }
  }
  return [...bySsid.values()].sort((left, right) => {
    if (left.connected !== right.connected) return left.connected ? -1 : 1;
    if (left.known !== right.known) return left.known ? -1 : 1;
    return right.signalQuality - left.signalQuality;
  });
}

export function networkErrorMessage(error: unknown): string {
  const raw = error instanceof Error ? error.message : String(error);
  const normalized = raw.toUpperCase();
  if (normalized.includes("WIFI_DISABLED"))
    return "El adaptador Wi-Fi está desactivado.";
  if (normalized.includes("NETWORK_NOT_FOUND"))
    return "La red ya no está disponible.";
  if (normalized.includes("WRONG_PASSWORD"))
    return "La contraseña parece incorrecta.";
  if (normalized.includes("NO_NETWORKS")) return "No se encontraron redes.";
  if (normalized.includes("NO_INTERNET"))
    return "Conectado a la red, pero sin acceso a Internet.";
  if (normalized.includes("NETWORK_OPERATION_REQUIRES_ADMIN"))
    return "Windows requiere permisos de administrador para cambiar este adaptador.";
  if (normalized.includes("NETWORK_OPERATION_CANCELLED"))
    return "La operación fue cancelada.";
  if (normalized.includes("UNAVAILABLE"))
    return "La conectividad de Windows no está disponible.";
  return "No se pudo completar la operación de red.";
}
