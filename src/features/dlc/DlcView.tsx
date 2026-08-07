import { useEffect, useMemo, useState, type ReactNode } from "react";
import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import {
  dlcStatusIcon,
  dlcStatusLabel,
  getGameDlc,
  type DlcItem,
} from "./dlc-data";

export function DlcView({ game }: { game: Game }) {
  const dlcItems = useMemo(() => getGameDlc(game), [game]);
  const [selectedId, setSelectedId] = useState(dlcItems[0]?.id ?? "");
  const [actionMessage, setActionMessage] = useState("");
  const selectedDlc =
    dlcItems.find((item) => item.id === selectedId) ?? dlcItems[0];

  useEffect(() => {
    setSelectedId(dlcItems[0]?.id ?? "");
    setActionMessage("");
  }, [dlcItems]);

  if (!selectedDlc) {
    return (
      <section
        className="details-dlc details-dlc-empty"
        aria-labelledby="dlc-heading"
      >
        <p className="eyebrow">Contenido adicional</p>
        <h2 id="dlc-heading">No hay DLC disponible</h2>
        <p>Este juego todavía no tiene contenido descargable registrado.</p>
      </section>
    );
  }

  const selectedIndex = dlcItems.findIndex(
    (item) => item.id === selectedDlc.id,
  );
  const selectedCardId = `details-dlc-item-${selectedDlc.id}`;
  const primaryLabel =
    selectedDlc.status === "installed"
      ? "Gestionar contenido"
      : selectedDlc.status === "owned"
        ? "Instalar contenido"
        : selectedDlc.contextualAction;
  const secondaryLabel =
    selectedDlc.status === "available" ? "Ver detalles" : "Abrir contenido";

  const handleAction = (label: string) => {
    setActionMessage(`${label}: ${selectedDlc.title}`);
  };

  return (
    <section className="details-dlc" aria-label="DLC">
      <div className="details-dlc-heading">
        <div>
          <p className="eyebrow">Contenido adicional</p>
        </div>
        <span className="details-dlc-count">
          {dlcItems.length} {dlcItems.length === 1 ? "elemento" : "elementos"}
        </span>
      </div>
      <div className="details-dlc-layout">
        <div className="details-dlc-list" role="list" aria-label="Lista de DLC">
          {dlcItems.map((item, index) => (
            <DlcListItem
              key={item.id}
              item={item}
              index={index}
              isSelected={item.id === selectedDlc.id}
              previousCardId={
                index > 0
                  ? `details-dlc-item-${dlcItems[index - 1]?.id ?? item.id}`
                  : undefined
              }
              nextCardId={
                index < dlcItems.length - 1
                  ? `details-dlc-item-${dlcItems[index + 1]?.id ?? item.id}`
                  : undefined
              }
              onSelect={() => {
                setSelectedId(item.id);
                setActionMessage("");
              }}
            />
          ))}
        </div>
        <aside className="details-dlc-detail" aria-live="polite">
          <div
            key={selectedDlc.id}
            className="details-dlc-detail-content"
            style={{ backgroundImage: `url("${selectedDlc.heroUrl}")` }}
          >
            <div className="details-dlc-detail-overlay" />
            <div className="details-dlc-detail-copy">
              <p className="eyebrow">{selectedDlc.contentType}</p>
              <h3>{selectedDlc.title}</h3>
              <p>{selectedDlc.description}</p>
              <div className="details-dlc-actions">
                <Focusable
                  focusId="details-dlc-primary"
                  scopeId="details"
                  className="primary-button details-dlc-primary"
                  navigation={{
                    left: selectedCardId,
                    up: "details-tab-dlc",
                    down: "details-dlc-secondary",
                  }}
                  onConfirm={() => handleAction(primaryLabel)}
                >
                  <span className="details-dlc-action-icon" aria-hidden="true">
                    {selectedDlc.status === "installed" ? "⚙" : "↓"}
                  </span>
                  {primaryLabel}
                </Focusable>
                <Focusable
                  focusId="details-dlc-secondary"
                  scopeId="details"
                  className="secondary-button details-dlc-secondary"
                  navigation={{
                    left: selectedCardId,
                    up: "details-dlc-primary",
                  }}
                  onConfirm={() => handleAction(secondaryLabel)}
                >
                  <span className="details-dlc-action-icon" aria-hidden="true">
                    ▷
                  </span>
                  {secondaryLabel}
                </Focusable>
              </div>
              {actionMessage && (
                <p className="details-dlc-message" role="status">
                  {actionMessage}
                </p>
              )}
            </div>
            <dl className="details-dlc-info">
              <InfoRow label="Estado">
                <span className={`details-dlc-status is-${selectedDlc.status}`}>
                  <span aria-hidden="true">
                    {dlcStatusIcon(selectedDlc.status)}
                  </span>
                  {dlcStatusLabel(selectedDlc.status)}
                </span>
              </InfoRow>
              <InfoRow label="Fecha de instalación">
                {selectedDlc.installationDate ?? "No instalado"}
              </InfoRow>
              <InfoRow label="Tamaño">{selectedDlc.size}</InfoRow>
              <InfoRow label="Versión">{selectedDlc.version}</InfoRow>
              <InfoRow label="Plataforma">{selectedDlc.platform}</InfoRow>
              <InfoRow label="Idioma">{selectedDlc.language}</InfoRow>
            </dl>
          </div>
        </aside>
      </div>
      <span className="visually-hidden" aria-live="polite">
        DLC seleccionado {selectedIndex + 1} de {dlcItems.length}:{" "}
        {selectedDlc.title}
      </span>
    </section>
  );
}

function DlcListItem({
  item,
  index,
  isSelected,
  previousCardId,
  nextCardId,
  onSelect,
}: {
  item: DlcItem;
  index: number;
  isSelected: boolean;
  previousCardId?: string;
  nextCardId?: string;
  onSelect: () => void;
}) {
  const focusId = `details-dlc-item-${item.id}`;

  return (
    <Focusable
      focusId={focusId}
      scopeId="details"
      className={`details-dlc-card${isSelected ? " is-selected" : ""}`}
      role="button"
      navigation={{
        up: index === 0 ? "details-tab-dlc" : previousCardId,
        down: nextCardId,
        right: "details-dlc-primary",
      }}
      ariaLabel={`${item.title}, ${dlcStatusLabel(item.status)}`}
      ariaPressed={isSelected}
      onFocus={onSelect}
      onConfirm={onSelect}
    >
      <span
        className="details-dlc-card-hero"
        style={{ backgroundImage: `url("${item.heroUrl}")` }}
        aria-hidden="true"
      />
      <span className="details-dlc-card-overlay" aria-hidden="true" />
      <span className="details-dlc-card-copy">
        <span className="details-dlc-card-type">{item.contentType}</span>
        <strong>{item.title}</strong>
        <span className="details-dlc-card-description">
          {item.shortDescription}
        </span>
        <span className="details-dlc-card-meta">
          <span className={`details-dlc-status is-${item.status}`}>
            <span aria-hidden="true">{dlcStatusIcon(item.status)}</span>
            {dlcStatusLabel(item.status)}
          </span>
          <span>{item.releaseDate}</span>
        </span>
      </span>
    </Focusable>
  );
}

function InfoRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="details-dlc-info-row">
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}
