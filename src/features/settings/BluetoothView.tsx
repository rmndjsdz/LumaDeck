import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { NavigationDialog } from "../../ui/navigation/layouts/NavigationDialog";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import {
  bluetoothDeviceFocusId,
  bluetoothErrorMessage,
  normalizeBluetoothDevices,
  type BluetoothDevice,
  type BluetoothSnapshot,
} from "./bluetooth-types";
import { bluetoothService } from "./bluetooth-service";

type BluetoothOperation =
  "refreshing" | "toggling" | "discovering" | "pairing" | "forgetting" | null;

export function BluetoothView() {
  const { engine } = useNavigation();
  const [snapshot, setSnapshot] = useState<BluetoothSnapshot | null>(null);
  const [operation, setOperation] = useState<BluetoothOperation>("refreshing");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [forgetTarget, setForgetTarget] = useState<BluetoothDevice | null>(
    null,
  );
  const discoveryActiveRef = useRef(false);

  const devices = useMemo(
    () => normalizeBluetoothDevices(snapshot?.devices ?? []),
    [snapshot?.devices],
  );
  const connectedDevices = devices.filter((device) => device.connected);
  const knownDevices = devices.filter(
    (device) => device.paired && !device.connected,
  );
  const discoveredDevices = devices.filter((device) => !device.paired);
  const busy = operation !== null;

  const refresh = useCallback(async (discover = false) => {
    try {
      const current = await bluetoothService.getSnapshot();
      setSnapshot(
        discover && current.enabled
          ? await bluetoothService.startDiscovery()
          : current,
      );
      if (discover) setErrorMessage(null);
    } catch (error) {
      setErrorMessage(bluetoothErrorMessage(error));
      void bluetoothService
        .recordClientDiagnostic(
          "error",
          "ui.refresh_error",
          `message=${String(error)}`,
        )
        .catch(() => undefined);
    } finally {
      setOperation((current) => (current === "refreshing" ? null : current));
    }
  }, []);

  useEffect(() => {
    void refresh(true);
  }, [refresh]);

  useEffect(() => {
    const interval = window.setInterval(
      () => void refresh(),
      snapshot?.discoveryActive ? 1000 : 3000,
    );
    return () => window.clearInterval(interval);
  }, [refresh, snapshot?.discoveryActive]);

  useEffect(() => {
    discoveryActiveRef.current = snapshot?.discoveryActive ?? false;
  }, [snapshot?.discoveryActive]);

  useEffect(() => {
    return () => {
      if (discoveryActiveRef.current) {
        void bluetoothService.stopDiscovery().catch(() => undefined);
      }
    };
  }, []);

  const runOperation = useCallback(
    async (
      kind: Exclude<BluetoothOperation, "refreshing" | null>,
      action: () => Promise<BluetoothSnapshot>,
    ) => {
      if (operation !== null) return;
      setOperation(kind);
      setErrorMessage(null);
      try {
        setSnapshot(await action());
      } catch (error) {
        setErrorMessage(bluetoothErrorMessage(error));
        void bluetoothService
          .recordClientDiagnostic(
            "error",
            "ui.operation_error",
            `operation=${kind}; message=${String(error)}`,
          )
          .catch(() => undefined);
      } finally {
        setOperation(null);
      }
    },
    [operation],
  );

  const toggleBluetooth = () => {
    if (!snapshot || !snapshot.available) return;
    void runOperation("toggling", () =>
      bluetoothService.setEnabled(!snapshot.enabled),
    );
  };

  const toggleDiscovery = () => {
    if (!snapshot?.enabled) return;
    void runOperation(
      "discovering",
      snapshot.discoveryActive
        ? () => bluetoothService.stopDiscovery()
        : () => bluetoothService.startDiscovery(),
    );
  };

  const pair = (device: BluetoothDevice) => {
    if (device.paired || !snapshot?.enabled) return;
    void runOperation("pairing", () => bluetoothService.pairDevice(device.id));
  };

  const openForget = (device: BluetoothDevice) => {
    if (busy) return;
    engine.prepareScopeOpen(
      "bluetooth-forget-dialog",
      `${bluetoothDeviceFocusId(device.id)}-forget`,
    );
    setForgetTarget(device);
  };

  const closeForget = useCallback(() => {
    setForgetTarget(null);
    return true;
  }, []);

  const forget = () => {
    if (!forgetTarget) return;
    const device = forgetTarget;
    closeForget();
    void runOperation("forgetting", () =>
      bluetoothService.unpairDevice(device.id),
    );
  };

  return (
    <section className="bluetooth-view" aria-label="Bluetooth">
      <div className={`bluetooth-status-card is-${statusClass(snapshot)}`}>
        <div>
          <p className="eyebrow">Bluetooth</p>
          <strong>{statusLabel(snapshot)}</strong>
          <span>
            {snapshot?.available
              ? `${connectedDevices.length} conectados · ${knownDevices.length + connectedDevices.length} conocidos`
              : "No se detectó un adaptador compatible"}
          </span>
        </div>
        <span className="bluetooth-status-dot" aria-hidden="true" />
      </div>

      {errorMessage && (
        <p className="settings-feedback is-error" role="alert">
          {errorMessage}
        </p>
      )}

      {!snapshot?.available ? (
        <article className="bluetooth-empty-panel">
          <p className="eyebrow">Bluetooth no disponible</p>
          <h2>No se encontró un adaptador Bluetooth compatible.</h2>
          <p>Windows no expone ningún radio Bluetooth en este equipo.</p>
          <Focusable
            focusId="bluetooth-refresh"
            scopeId="settings-shell"
            className="settings-button secondary"
            onConfirm={() => void refresh()}
            disabled={busy}
          >
            {operation === "refreshing" ? "Comprobando…" : "Volver a comprobar"}
          </Focusable>
        </article>
      ) : (
        <>
          <div className="bluetooth-actions-panel">
            <div>
              <p className="eyebrow">Radio</p>
              <strong>
                {snapshot.enabled
                  ? "Listo para conectar"
                  : "Bluetooth está desactivado"}
              </strong>
            </div>
            <div className="bluetooth-action-row">
              <Focusable
                focusId="bluetooth-toggle"
                scopeId="settings-shell"
                className="settings-button secondary"
                onConfirm={toggleBluetooth}
                disabled={busy}
              >
                {operation === "toggling"
                  ? "Cambiando…"
                  : snapshot.enabled
                    ? "Desactivar Bluetooth"
                    : "Activar Bluetooth"}
              </Focusable>
              <Focusable
                focusId="bluetooth-discovery"
                scopeId="settings-shell"
                className="settings-button primary"
                onConfirm={toggleDiscovery}
                disabled={busy || !snapshot.enabled}
              >
                {operation === "discovering" || snapshot.discoveryActive
                  ? "Cancelar búsqueda"
                  : "Buscar dispositivos"}
              </Focusable>
            </div>
          </div>

          <BluetoothDeviceSection
            title="Conectados"
            devices={connectedDevices}
            emptyMessage="No hay dispositivos conectados."
            busy={busy}
            onPair={pair}
            onForget={openForget}
          />
          <BluetoothDeviceSection
            title="Conocidos"
            devices={knownDevices}
            emptyMessage="Los dispositivos emparejados aparecerán aquí."
            busy={busy}
            onPair={pair}
            onForget={openForget}
          />
          {snapshot.discoveryActive && (
            <BluetoothDeviceSection
              title="Encontrados"
              devices={discoveredDevices}
              emptyMessage="Buscando dispositivos cercanos…"
              busy={busy}
              onPair={pair}
              onForget={openForget}
            />
          )}
        </>
      )}

      {forgetTarget && (
        <div className="bluetooth-dialog-backdrop">
          <NavigationDialog
            scopeId="bluetooth-forget-dialog"
            initialFocusId="bluetooth-forget-cancel"
            onBack={closeForget}
          >
            <p className="eyebrow">Bluetooth · Confirmar</p>
            <h2>¿Olvidar {forgetTarget.name}?</h2>
            <p className="settings-muted">
              Tendrás que volver a emparejarlo para usarlo nuevamente.
            </p>
            <div className="bluetooth-dialog-actions">
              <Focusable
                focusId="bluetooth-forget-confirm"
                scopeId="bluetooth-forget-dialog"
                className="settings-button primary"
                onConfirm={forget}
              >
                Olvidar dispositivo
              </Focusable>
              <Focusable
                focusId="bluetooth-forget-cancel"
                scopeId="bluetooth-forget-dialog"
                className="settings-button"
                onConfirm={closeForget}
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

function BluetoothDeviceSection({
  title,
  devices,
  emptyMessage,
  busy,
  onPair,
  onForget,
}: {
  title: string;
  devices: readonly BluetoothDevice[];
  emptyMessage: string;
  busy: boolean;
  onPair: (device: BluetoothDevice) => void;
  onForget: (device: BluetoothDevice) => void;
}) {
  return (
    <section className="bluetooth-device-section" aria-label={title}>
      <div className="bluetooth-section-heading">
        <div>
          <p className="eyebrow">Bluetooth</p>
          <h2>{title}</h2>
        </div>
        <span className="settings-muted">{devices.length}</span>
      </div>
      {devices.length === 0 ? (
        <p className="empty-state">{emptyMessage}</p>
      ) : (
        <NavigationGrid
          groupId={`bluetooth-${title.toLocaleLowerCase()}`}
          columns={1}
          itemCount={devices.length}
          regionId="settings-content"
          entryFocusId={bluetoothDeviceFocusId(devices[0]!.id)}
          exitFocusId="bluetooth-discovery"
          className="bluetooth-device-list"
        >
          {devices.map((device, index) => (
            <div className="bluetooth-device-row" key={device.id}>
              <Focusable
                focusId={bluetoothDeviceFocusId(device.id)}
                scopeId="settings-shell"
                gridIndex={index}
                className={`bluetooth-device-item ${device.connected ? "is-connected" : ""}`}
                onConfirm={() => onPair(device)}
                disabled={busy || device.paired}
                ariaLabel={`${device.name}, ${categoryLabel(device.deviceClass)}, ${deviceStateLabel(device)}`}
              >
                <span className="bluetooth-device-icon" aria-hidden="true">
                  {categoryIcon(device.deviceClass)}
                </span>
                <span className="bluetooth-device-copy">
                  <strong>{device.name}</strong>
                  <small>
                    {categoryLabel(device.deviceClass)} ·{" "}
                    {deviceStateLabel(device)}
                    {device.batteryLevel != null
                      ? ` · ${device.batteryLevel}%`
                      : ""}
                  </small>
                </span>
                <span className="bluetooth-device-action">
                  {!device.paired
                    ? "Emparejar"
                    : device.connected
                      ? "Conectado"
                      : "Emparejado"}
                </span>
              </Focusable>
              {device.paired && (
                <Focusable
                  focusId={`${bluetoothDeviceFocusId(device.id)}-forget`}
                  scopeId="settings-shell"
                  className="bluetooth-forget-button"
                  onConfirm={() => onForget(device)}
                  disabled={busy}
                  ariaLabel={`Olvidar ${device.name}`}
                >
                  Olvidar
                </Focusable>
              )}
            </div>
          ))}
        </NavigationGrid>
      )}
    </section>
  );
}

function statusClass(snapshot: BluetoothSnapshot | null): string {
  if (!snapshot?.available) return "unavailable";
  return snapshot.enabled ? "enabled" : "disabled";
}

function statusLabel(snapshot: BluetoothSnapshot | null): string {
  if (!snapshot) return "Comprobando…";
  if (!snapshot.available) return "No disponible";
  return snapshot.enabled ? "Activado" : "Desactivado";
}

function deviceStateLabel(device: BluetoothDevice): string {
  if (device.connected) return "Conectado";
  if (device.paired) return "Emparejado";
  return "No emparejado";
}

function categoryLabel(category: BluetoothDevice["deviceClass"]): string {
  return {
    gamepad: "Gamepad",
    headphones: "Auriculares",
    headset: "Headset",
    speaker: "Altavoz",
    keyboard: "Teclado",
    mouse: "Mouse",
    phone: "Teléfono",
    computer: "Computadora",
    other: "Otro",
  }[category];
}

function categoryIcon(category: BluetoothDevice["deviceClass"]): string {
  return {
    gamepad: "⌘",
    headphones: "◖",
    headset: "◉",
    speaker: "◒",
    keyboard: "▦",
    mouse: "◌",
    phone: "▯",
    computer: "▣",
    other: "◇",
  }[category];
}
