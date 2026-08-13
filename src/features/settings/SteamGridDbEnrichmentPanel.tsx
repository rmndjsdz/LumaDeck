import { useCallback, useEffect, useMemo, useState } from "react";
import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import {
  providerSettingsService,
  ProviderSettingsError,
} from "./provider-settings-service";
import type {
  ArtworkEnrichmentScope,
  ArtworkEnrichmentSlot,
  ArtworkEnrichmentStatus,
} from "./settings-types";
import {
  ArtworkTypeIcon,
  type ArtworkTypeIconVariant,
} from "./ArtworkTypeIcon";

const ARTWORK_SLOTS: readonly {
  id: ArtworkEnrichmentSlot;
  variant: ArtworkTypeIconVariant;
  primary: string;
  secondary: string;
}[] = [
  {
    id: "grid_horizontal",
    variant: "horizontal",
    primary: "Grid",
    secondary: "Horizontal",
  },
  {
    id: "grid_vertical",
    variant: "vertical",
    primary: "Grid",
    secondary: "Vertical",
  },
  {
    id: "grid_square",
    variant: "square",
    primary: "Grid",
    secondary: "Square",
  },
  { id: "hero", variant: "hero", primary: "Hero", secondary: "" },
  { id: "logo", variant: "logo", primary: "Logo", secondary: "PNG/alpha" },
];

const DEFAULT_SLOTS = ARTWORK_SLOTS.map(({ id }) => id);
const CONCURRENCY_VALUES = [2, 4, 6] as const;
const RESOLUTION_VALUES = [2048, 4096, 8192] as const;

export function SteamGridDbEnrichmentPanel({
  games,
  onStatusChange,
}: {
  games: readonly Game[];
  onStatusChange?: (status: ArtworkEnrichmentStatus) => void;
}) {
  const [scope, setScope] = useState<ArtworkEnrichmentScope>("only_non_steam");
  const [slots, setSlots] = useState<ArtworkEnrichmentSlot[]>(DEFAULT_SLOTS);
  const [concurrency, setConcurrency] = useState(4);
  const [maxDimension, setMaxDimension] = useState(4096);
  const [status, setStatus] = useState<ArtworkEnrichmentStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const publishStatus = useCallback(
    (nextStatus: ArtworkEnrichmentStatus) => {
      setStatus(nextStatus);
      onStatusChange?.(nextStatus);
    },
    [onStatusChange],
  );

  const refreshStatus = useCallback(async () => {
    try {
      publishStatus(await providerSettingsService.getArtworkEnrichmentStatus());
    } catch {
      // Keep the provider card usable if the optional status read fails.
    }
  }, [publishStatus]);

  useEffect(() => {
    void refreshStatus();
    const timer = window.setInterval(() => void refreshStatus(), 800);
    return () => window.clearInterval(timer);
  }, [refreshStatus]);

  const isRunning = status?.status === "running";
  const selectedCount = slots.length;
  const eligibleGames = useMemo(
    () =>
      scope === "only_non_steam"
        ? games.filter((game) => game.source !== "steam").length
        : games.length,
    [games, scope],
  );

  const cycleScope = () =>
    setScope((current) =>
      current === "only_non_steam" ? "all" : "only_non_steam",
    );

  const cycleValue = <T extends number>(
    values: readonly T[],
    current: T,
    setter: (value: T) => void,
  ) => {
    const index = values.indexOf(current);
    setter(values[(index + 1) % values.length]);
  };

  const toggleSlot = (slot: ArtworkEnrichmentSlot) => {
    setSlots((current) =>
      current.includes(slot)
        ? current.filter((value) => value !== slot)
        : [...current, slot],
    );
  };

  const start = async () => {
    if (isRunning || slots.length === 0) return;
    setError(null);
    try {
      publishStatus(
        await providerSettingsService.startArtworkEnrichment({
          gameIds: games.map((game) => game.id),
          scope,
          slots,
          maxDimension,
          concurrency,
        }),
      );
    } catch (reason) {
      setError(
        reason instanceof ProviderSettingsError
          ? "No se pudo iniciar el proceso de artwork."
          : "SteamGridDB no está disponible en este momento.",
      );
    }
  };

  const cancel = async () => {
    try {
      publishStatus(await providerSettingsService.cancelArtworkEnrichment());
    } catch {
      setError("No se pudo cancelar el proceso de artwork.");
    }
  };

  const progress =
    status && status.totalGames > 0
      ? Math.round((status.processedGames / status.totalGames) * 100)
      : 0;

  return (
    <article className="settings-panel artwork-enrichment-panel">
      <div className="artwork-panel-heading">
        <span className="artwork-panel-icon" aria-hidden="true">
          <ArtworkTypeIcon variant="hero" />
        </span>
        <div>
          <p className="eyebrow">Arte automático para biblioteca</p>
          <h2>Completar arte faltante</h2>
          <p className="settings-helper">
            Busca y descarga automáticamente el mejor arte disponible en
            SteamGridDB para los juegos seleccionados.
          </p>
        </div>
      </div>
      <div className="artwork-protection-note">
        <span className="artwork-protection-icon" aria-hidden="true" />
        <div>
          <strong>
            No se reemplazará ningún arte personalizado o seleccionado
            manualmente.
          </strong>
          <span>Solo se completará lo que esté faltante.</span>
        </div>
      </div>

      <div className="artwork-scope-row">
        <div>
          <p className="settings-field-label">
            Alcance <span aria-hidden="true">ⓘ</span>
          </p>
          <Focusable
            focusId="steamgriddb-enrichment-scope"
            scopeId="settings-shell"
            className="artwork-select-button"
            ariaLabel="Alcance del proceso"
            onConfirm={cycleScope}
          >
            <span aria-hidden="true">⌁</span>
            {scope === "only_non_steam"
              ? "Solo juegos emulados / no-Steam"
              : "Todos los compatibles"}
            <span aria-hidden="true">⌄</span>
          </Focusable>
          <p className="settings-helper">
            {scope === "only_non_steam"
              ? "Procesará únicamente juegos que no provienen de Steam (" +
                eligibleGames +
                ")."
              : "Procesará todos los juegos compatibles (" +
                eligibleGames +
                ")."}
          </p>
        </div>
        <div className="artwork-types-field">
          <p className="settings-field-label">
            Tipos de arte a descargar{" "}
            <span>{selectedCount}/5 seleccionados</span>
          </p>
          <div className="artwork-slot-grid">
            {ARTWORK_SLOTS.map((slot) => {
              const selected = slots.includes(slot.id);
              return (
                <Focusable
                  key={slot.id}
                  focusId={"steamgriddb-enrichment-" + slot.id}
                  scopeId="settings-shell"
                  className={
                    "artwork-slot-card " + (selected ? "is-selected" : "")
                  }
                  ariaPressed={selected}
                  onConfirm={() => toggleSlot(slot.id)}
                >
                  <ArtworkTypeIcon variant={slot.variant} />
                  <strong>{slot.primary}</strong>
                  {slot.secondary && <small>{slot.secondary}</small>}
                  {selected && (
                    <span className="artwork-slot-check" aria-hidden="true">
                      ✓
                    </span>
                  )}
                </Focusable>
              );
            })}
          </div>
        </div>
      </div>

      <div className="artwork-enrichment-options">
        <div>
          <p className="settings-field-label">Preferencia de calidad</p>
          <Focusable
            focusId="steamgriddb-enrichment-quality"
            scopeId="settings-shell"
            className="artwork-select-button"
            ariaLabel="Preferencia de calidad"
            onConfirm={() => undefined}
          >
            Mayor densidad disponible (sin upscale)
            <span aria-hidden="true">⌄</span>
          </Focusable>
        </div>
        <div>
          <p className="settings-field-label">Descargas simultáneas</p>
          <Focusable
            focusId="steamgriddb-enrichment-concurrency"
            scopeId="settings-shell"
            className="artwork-select-button"
            ariaLabel="Descargas simultáneas"
            onConfirm={() =>
              cycleValue(CONCURRENCY_VALUES, concurrency, setConcurrency)
            }
          >
            {concurrency}
            <span aria-hidden="true">⌃⌄</span>
          </Focusable>
        </div>
        <div>
          <p className="settings-field-label">
            Límite de resolución (por lado mayor)
          </p>
          <Focusable
            focusId="steamgriddb-enrichment-resolution"
            scopeId="settings-shell"
            className="artwork-select-button"
            ariaLabel="Límite de resolución"
            onConfirm={() =>
              cycleValue(RESOLUTION_VALUES, maxDimension, setMaxDimension)
            }
          >
            Hasta {maxDimension} px
            <span aria-hidden="true">⌄</span>
          </Focusable>
        </div>
      </div>

      <div className="artwork-enrichment-actions">
        {isRunning ? (
          <div className="artwork-progress" role="status">
            <div className="artwork-progress-heading">
              <strong>Completando arte</strong>
              <span>{progress}%</span>
            </div>
            <div className="artwork-progress-bar">
              <span style={{ width: String(progress) + "%" }} />
            </div>
            <p>
              {status?.processedGames ?? 0} / {status?.totalGames ?? 0} juegos
              {status?.currentGame ? " · " + status.currentGame : ""}
            </p>
            {status?.currentArtwork && <small>{status.currentArtwork}</small>}
            <Focusable
              focusId="steamgriddb-enrichment-cancel"
              scopeId="settings-shell"
              className="settings-button danger"
              onConfirm={() => void cancel()}
            >
              Cancelar proceso
            </Focusable>
          </div>
        ) : (
          <Focusable
            focusId="steamgriddb-enrichment-start"
            scopeId="settings-shell"
            className="settings-action settings-action-primary"
            disabled={selectedCount === 0 || eligibleGames === 0}
            onConfirm={() => void start()}
          >
            <span aria-hidden="true">▶</span> Iniciar proceso
          </Focusable>
        )}
        {!isRunning && (
          <Focusable
            focusId="steamgriddb-enrichment-advanced"
            scopeId="settings-shell"
            className="artwork-advanced-button"
            onConfirm={() => undefined}
          >
            <span aria-hidden="true">⚙</span> Avanzado
          </Focusable>
        )}
      </div>
      {!isRunning && (
        <p className="artwork-duration-helper">
          Este proceso puede tomar desde unos minutos hasta varias horas
          dependiendo del tamaño de tu biblioteca.
        </p>
      )}
      {error && (
        <p className="settings-feedback is-error" role="alert">
          {error}
        </p>
      )}
    </article>
  );
}

export function SteamGridDbEnrichmentSummary({
  status,
}: {
  status: ArtworkEnrichmentStatus | null;
}) {
  return (
    <>
      <article className="settings-panel artwork-last-process-panel">
        <strong>Último proceso</strong>
        {status?.completedAt ? (
          <>
            <p>{status.status}</p>
            <small>
              {status.processedGames} juegos · {status.downloadedAssets} assets
            </small>
          </>
        ) : (
          <div className="artwork-empty-process">
            <span className="artwork-process-icon" aria-hidden="true" />
            <div>
              <p>Sin ejecuciones previas</p>
              <small>
                Completa arte faltante para iniciar el primer proceso.
              </small>
            </div>
          </div>
        )}
      </article>
      <article className="settings-panel artwork-results-panel">
        <strong>
          Resumen de resultados <small>(ejemplo)</small>
        </strong>
        <div className="artwork-summary">
          <span className="summary-dot is-green" />{" "}
          <b>{status?.downloadedAssets ?? 0}</b>
          <span>Arte descargado / actualizado</span>
          <span className="summary-dot is-blue" />{" "}
          <b>{status?.alreadyCompleteGames ?? 0}</b>
          <span>Ya estaban completos</span>
          <span className="summary-dot is-yellow" />{" "}
          <b>{status?.noResultGames ?? 0}</b>
          <span>Sin resultados</span>
          <span className="summary-dot is-orange" />{" "}
          <b>{status?.ambiguousGames ?? 0}</b>
          <span>Coincidencias ambiguas</span>
          <span className="summary-dot is-red" />{" "}
          <b>{status?.errorCount ?? 0}</b>
          <span>Errores</span>
        </div>
        <button type="button" className="artwork-review-link" disabled>
          Ver pendientes de revisión <span aria-hidden="true">›</span>
        </button>
      </article>
    </>
  );
}
