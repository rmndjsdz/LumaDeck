import { useEffect, useState } from "react";
import { networkService } from "./network-service";
import { selectActiveConnection, type NetworkSnapshot } from "./network-types";

type ConnectionKind = "ethernet" | "wifi" | "offline" | "connecting" | "error";

export function NetworkConnectionIndicator() {
  const [kind, setKind] = useState<ConnectionKind>("offline");

  useEffect(() => {
    let disposed = false;

    const updateConnection = async () => {
      if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
        return;
      }

      try {
        const snapshot = await networkService.getSnapshot();
        if (!disposed) setKind(connectionKind(snapshot));
      } catch {
        if (!disposed) setKind("offline");
      }
    };

    void updateConnection();
    const interval = window.setInterval(() => void updateConnection(), 10000);

    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, []);

  const label = connectionLabel(kind);

  return (
    <span
      className={`shell-connection-indicator is-${kind}`}
      role="status"
      aria-label={label}
      title={label}
    >
      <ConnectionIcon kind={kind} />
      <span className="shell-connection-label">{label}</span>
    </span>
  );
}

function connectionKind(snapshot: NetworkSnapshot): ConnectionKind {
  if (
    snapshot.internetState === "connecting" ||
    snapshot.adapters.some((adapter) => adapter.state === "connecting")
  ) {
    return "connecting";
  }

  if (snapshot.internetState === "connected-no-internet") return "error";

  const activeConnection = selectActiveConnection(snapshot.adapters);
  const activeType = activeConnection?.type ?? snapshot.activeConnectionType;

  return activeType === "ethernet" || activeType === "wifi"
    ? activeType
    : "offline";
}

function connectionLabel(kind: ConnectionKind): string {
  if (kind === "ethernet") return "Conexi\u00f3n por cable";
  if (kind === "wifi") return "Conexi\u00f3n por Wi-Fi";
  if (kind === "connecting") return "Conectando";
  if (kind === "error") return "Error de red";
  return "Sin conexi\u00f3n";
}

function ConnectionIcon({ kind }: { kind: ConnectionKind }) {
  const assetByKind: Record<ConnectionKind, string> = {
    ethernet: "/assets/network/network-ethernet.png",
    wifi: "/assets/network/network-wifi.png",
    offline: "/assets/network/network-offline.png",
    connecting: "/assets/network/network-connecting.png",
    error: "/assets/network/network-error.png",
  };

  return (
    <img
      className="shell-connection-icon"
      src={assetByKind[kind]}
      alt=""
      draggable={false}
    />
  );
}
