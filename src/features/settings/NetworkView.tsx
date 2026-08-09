import { useCallback, useEffect, useMemo, useState } from "react";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { GamepadTextInput } from "../../ui/keyboard/GamepadTextInput";
import { NavigationDialog } from "../../ui/navigation/layouts/NavigationDialog";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import {
  networkErrorMessage,
  normalizeWifiNetworks,
  selectActiveConnection,
  type NetworkAdapter,
  type NetworkSnapshot,
  type WifiNetwork,
} from "./network-types";
import { networkService } from "./network-service";

type NetworkOperation = "scanning" | "toggling" | "connecting" | null;

export function NetworkView() {
  const { engine } = useNavigation();
  const [snapshot, setSnapshot] = useState<NetworkSnapshot | null>(null);
  const [operation, setOperation] = useState<NetworkOperation>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [selectedNetwork, setSelectedNetwork] = useState<WifiNetwork | null>(
    null,
  );
  const [passwordDraft, setPasswordDraft] = useState("");
  const [passwordVisible, setPasswordVisible] = useState(false);
  const [connectingSsid, setConnectingSsid] = useState<string | null>(null);

  const wifiAdapters = useMemo(
    () => snapshot?.adapters.filter((adapter) => adapter.type === "wifi") ?? [],
    [snapshot?.adapters],
  );
  const wifiNetworks = useMemo(
    () => normalizeWifiNetworks(snapshot?.wifiNetworks ?? []),
    [snapshot?.wifiNetworks],
  );
  const activeConnection = useMemo(
    () => selectActiveConnection(snapshot?.adapters ?? []),
    [snapshot?.adapters],
  );

  const refresh = useCallback(async (scan = false) => {
    if (scan) {
      setOperation("scanning");
      setErrorMessage(null);
    }
    try {
      setSnapshot(
        await (scan ? networkService.scanWifi() : networkService.getSnapshot()),
      );
    } catch (error) {
      setErrorMessage(networkErrorMessage(error));
    } finally {
      if (scan) setOperation(null);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const interval = window.setInterval(() => void refresh(), 5000);
    return () => window.clearInterval(interval);
  }, [refresh]);

  useEffect(() => {
    if (!connectingSsid) return;
    const timeout = window.setTimeout(() => setConnectingSsid(null), 7000);
    return () => window.clearTimeout(timeout);
  }, [connectingSsid]);

  const updateFromOperation = useCallback(
    async (
      action: () => Promise<NetworkSnapshot>,
      operationName: NetworkOperation,
    ) => {
      setOperation(operationName);
      setErrorMessage(null);
      try {
        setSnapshot(await action());
      } catch (error) {
        setErrorMessage(networkErrorMessage(error));
      } finally {
        setOperation(null);
      }
    },
    [],
  );

  const closePasswordDialog = useCallback(() => {
    setSelectedNetwork(null);
    setPasswordDraft("");
    setPasswordVisible(false);
    return true;
  }, []);

  const connect = useCallback(
    (network: WifiNetwork, password?: string) => {
      const adapterId = network.interfaceId;
      if (!adapterId) {
        setErrorMessage("El adaptador Wi-Fi no está disponible.");
        return;
      }
      setConnectingSsid(network.ssid);
      void updateFromOperation(
        () => networkService.connectWifi(adapterId, network.ssid, password),
        "connecting",
      );
    },
    [updateFromOperation],
  );

  const handleNetwork = (network: WifiNetwork) => {
    if (wifiAdapters.length === 0 || operation) return;
    if (network.connected) {
      void updateFromOperation(
        () => networkService.disconnectWifi(network.interfaceId),
        "connecting",
      );
      return;
    }
    if (network.known || network.security === "open") {
      connect(network);
      return;
    }
    engine.prepareScopeOpen(
      "network-connect-dialog",
      `network-wifi-${networkFocusKey(network)}`,
    );
    setSelectedNetwork(network);
  };

  const forget = (network: WifiNetwork) => {
    if (wifiAdapters.length === 0 || operation) return;
    void updateFromOperation(
      () => networkService.forgetWifi(network.interfaceId, network.ssid),
      "toggling",
    );
  };

  const submitPassword = () => {
    if (!selectedNetwork || !passwordDraft) return;
    const network = selectedNetwork;
    closePasswordDialog();
    connect(network, passwordDraft);
    setPasswordDraft("");
  };

  return (
    <section className="network-view" aria-label="Red e Internet">
      <article
        className={`network-status-card is-${snapshot?.internetState ?? "disconnected"}`}
      >
        <div>
          <p className="eyebrow">Internet</p>
          <strong>{internetLabel(snapshot?.internetState)}</strong>
          <span>
            {activeConnection
              ? `Conexión activa · ${adapterTypeLabel(activeConnection.type)}`
              : "Sin conexión activa"}
          </span>
        </div>
        <span className="network-status-dot" aria-hidden="true" />
      </article>

      {errorMessage && (
        <p className="settings-feedback is-error" role="alert">
          {errorMessage}
        </p>
      )}
      {connectingSsid && (
        <p className="settings-feedback" role="status">
          Conectando a {connectingSsid}…
        </p>
      )}

      <div className="network-adapter-grid">
        {snapshot?.adapters
          .filter((adapter) => adapter.type === "ethernet")
          .map((adapter) => (
            <AdapterCard
              key={adapter.id}
              adapter={adapter}
              busy={operation !== null}
              onToggle={() =>
                void updateFromOperation(
                  () =>
                    networkService.setAdapterEnabled(
                      adapter.id,
                      adapter.state === "disabled",
                    ),
                  "toggling",
                )
              }
            />
          ))}
        {wifiAdapters.map((adapter) => (
          <article
            className="network-panel network-wifi-panel"
            key={adapter.id}
          >
            <div className="network-panel-heading">
              <div>
                <p className="eyebrow">Wi-Fi</p>
                <h2>{adapter.name}</h2>
              </div>
              <span className={`network-state-pill is-${adapter.state}`}>
                {adapterStateLabel(adapter.state)}
              </span>
            </div>
            <div className="network-action-row">
              <Focusable
                focusId={`network-wifi-toggle-${adapterFocusKey(adapter)}`}
                scopeId="settings-shell"
                className="settings-button secondary"
                onConfirm={() =>
                  void updateFromOperation(
                    () =>
                      networkService.setWifiEnabled(
                        adapter.id,
                        adapter.state === "disabled",
                      ),
                    "toggling",
                  )
                }
                ariaLabel={
                  adapter.state === "disabled"
                    ? `Activar ${adapter.name}`
                    : `Desactivar ${adapter.name}`
                }
              >
                {adapter.state === "disabled"
                  ? "Activar Wi-Fi"
                  : "Desactivar Wi-Fi"}
              </Focusable>
            </div>
          </article>
        ))}
      </div>

      <section
        className="network-panel network-networks-panel"
        aria-labelledby="wifi-networks-heading"
      >
        <div className="network-panel-heading">
          <div>
            <p className="eyebrow">Wi-Fi</p>
            <h2 id="wifi-networks-heading">Redes disponibles</h2>
          </div>
          <div className="network-action-row">
            <span className="settings-muted">{wifiNetworks.length} redes</span>
            <Focusable
              focusId="network-refresh"
              scopeId="settings-shell"
              className="settings-button primary"
              onConfirm={() => void refresh(true)}
              disabled={operation !== null}
            >
              {operation === "scanning" ? "Buscando…" : "Buscar redes"}
            </Focusable>
          </div>
        </div>
        {wifiNetworks.length === 0 ? (
          <p className="empty-state">
            {snapshot?.wifiEnabled === false
              ? "Activa Wi-Fi para buscar redes."
              : "Busca redes para ver la disponibilidad cercana."}
          </p>
        ) : (
          <NavigationGrid
            groupId="network-wifi-networks"
            columns={1}
            itemCount={wifiNetworks.length}
            regionId="settings-content"
            entryFocusId={`network-wifi-${networkFocusKey(wifiNetworks[0]!)}`}
            exitFocusId="network-refresh"
            className="network-list"
          >
            {wifiNetworks.map((network, index) => (
              <div
                className="network-list-row"
                key={`${network.interfaceId}-${network.ssid}`}
              >
                <Focusable
                  focusId={`network-wifi-${networkFocusKey(network)}`}
                  scopeId="settings-shell"
                  gridIndex={index}
                  className={`network-list-item ${network.connected ? "is-connected" : ""}`}
                  onConfirm={() => handleNetwork(network)}
                  disabled={operation !== null}
                  ariaLabel={`${network.ssid}, ${network.security === "secured" ? "red segura" : "red abierta"}, señal ${network.signalQuality}%`}
                >
                  <span className="wifi-signal" aria-hidden="true">
                    {signalBars(network.signalQuality)}
                  </span>
                  <span className="network-list-copy">
                    <strong>{network.ssid}</strong>
                    <small>
                      {network.connected
                        ? "Conectada"
                        : network.known
                          ? "Red conocida"
                          : network.security === "secured"
                            ? "Protegida"
                            : "Abierta"}
                    </small>
                  </span>
                  <span className="network-list-action">
                    {network.connected ? "Desconectar" : "Conectar"}
                  </span>
                </Focusable>
                {network.known && !network.connected && (
                  <Focusable
                    focusId={`network-forget-${networkFocusKey(network)}`}
                    scopeId="settings-shell"
                    className="network-forget-button"
                    onConfirm={() => forget(network)}
                    ariaLabel={`Olvidar ${network.ssid}`}
                  >
                    Olvidar
                  </Focusable>
                )}
              </div>
            ))}
          </NavigationGrid>
        )}
      </section>

      {selectedNetwork && (
        <div className="network-dialog-backdrop">
          <NavigationDialog
            scopeId="network-connect-dialog"
            initialFocusId="network-password-input"
            onBack={closePasswordDialog}
          >
            <p className="eyebrow">Wi-Fi · Conectar</p>
            <h2>{selectedNetwork.ssid}</h2>
            <p className="settings-muted">
              Introduce la contraseña. Windows gestionará el perfil seguro.
            </p>
            <GamepadTextInput
              focusId="network-password-input"
              scopeId="network-connect-dialog"
              value={passwordDraft}
              onChange={setPasswordDraft}
              className="settings-input network-password-input"
              ariaLabel="Contraseña Wi-Fi"
              placeholder="Contraseña"
              secure={!passwordVisible}
              maxLength={128}
            />
            <div className="network-dialog-actions">
              <Focusable
                focusId="network-password-visibility"
                scopeId="network-connect-dialog"
                className="settings-button secondary"
                onConfirm={() => setPasswordVisible((visible) => !visible)}
              >
                {passwordVisible ? "Ocultar contraseña" : "Mostrar contraseña"}
              </Focusable>
              <Focusable
                focusId="network-connect-confirm"
                scopeId="network-connect-dialog"
                className="settings-button primary"
                onConfirm={submitPassword}
                disabled={!passwordDraft}
              >
                Conectar
              </Focusable>
              <Focusable
                focusId="network-connect-cancel"
                scopeId="network-connect-dialog"
                className="settings-button"
                onConfirm={closePasswordDialog}
              >
                Cancelar
              </Focusable>
            </div>
          </NavigationDialog>
        </div>
      )}
    </section>
  );
}

function AdapterCard({
  adapter,
  busy,
  onToggle,
}: {
  adapter: NetworkAdapter;
  busy: boolean;
  onToggle: () => void;
}) {
  return (
    <article className="network-panel network-ethernet-panel">
      <div className="network-panel-heading">
        <div>
          <p className="eyebrow">Ethernet</p>
          <h2>{adapter.name}</h2>
        </div>
        <span className={`network-state-pill is-${adapter.state}`}>
          {adapterStateLabel(adapter.state)}
        </span>
      </div>
      <div className="network-detail-grid">
        <NetworkDetail
          label="Velocidad"
          value={adapter.linkSpeed ?? "No disponible"}
        />
        <NetworkDetail label="IPv4" value={adapter.ipv4 ?? "No asignada"} />
        <NetworkDetail
          label="Gateway"
          value={adapter.gateway ?? "No asignado"}
        />
        <NetworkDetail
          label="DNS"
          value={adapter.dns.length > 0 ? adapter.dns.join(", ") : "Automático"}
        />
        <NetworkDetail label="MAC" value={adapter.mac ?? "No disponible"} />
        <Focusable
          focusId={`network-ethernet-toggle-${adapterFocusKey(adapter)}`}
          scopeId="settings-shell"
          className="settings-button secondary network-adapter-toggle"
          onConfirm={onToggle}
          disabled={busy}
          ariaLabel={
            adapter.state === "disabled"
              ? `Activar red cableada ${adapter.name}`
              : `Desactivar red cableada ${adapter.name}`
          }
        >
          {adapter.state === "disabled"
            ? "Activar red cableada"
            : "Desactivar red cableada"}
        </Focusable>
      </div>
    </article>
  );
}

function NetworkDetail({ label, value }: { label: string; value: string }) {
  return (
    <div className="network-detail">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function networkFocusKey(network: WifiNetwork): string {
  return encodeURIComponent(`${network.interfaceId}-${network.ssid}`).replace(
    /%/g,
    "_",
  );
}

function adapterFocusKey(adapter: NetworkAdapter): string {
  return encodeURIComponent(adapter.id).replace(/%/g, "_");
}

function signalBars(quality: number): string {
  const bars = Math.max(1, Math.min(4, Math.ceil(quality / 25)));
  return "▮".repeat(bars) + "▯".repeat(4 - bars);
}

function internetLabel(
  state: NetworkSnapshot["internetState"] | undefined,
): string {
  if (state === "connected") return "Conectado a Internet";
  if (state === "connected-no-internet") return "Conectado sin Internet";
  if (state === "connecting") return "Conectando…";
  return "Desconectado";
}

function adapterStateLabel(state: NetworkAdapter["state"]): string {
  if (state === "connected") return "Conectado";
  if (state === "connected-no-internet") return "Sin Internet";
  if (state === "disabled") return "Desactivado";
  if (state === "connecting") return "Conectando…";
  return "Desconectado";
}

function adapterTypeLabel(type: NetworkAdapter["type"]): string {
  if (type === "ethernet") return "Ethernet";
  if (type === "wifi") return "Wi-Fi";
  return "Otra interfaz";
}
