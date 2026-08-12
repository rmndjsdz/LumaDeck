import { useCallback, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { NavigationDialog } from "../../ui/navigation/layouts/NavigationDialog";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { MediaImage } from "../../ui/performance/MediaImage";
import {
  capabilitySourceLabel,
  capabilityValueLabel,
  gameCapabilitiesService,
} from "./game-capabilities-service";
import type {
  GameCapabilityKind,
  GameCapabilityOverrideState,
  ResolvedCapability,
  ResolvedGameCapabilities,
} from "./game-capabilities-types";
import { GraphicsProfilePanel } from "../graphics-profile/GraphicsProfilePanel";

const capabilityRows: readonly {
  kind: GameCapabilityKind;
  label: string;
  key: keyof Pick<
    ResolvedGameCapabilities,
    "nativeHdr" | "highFidelityUpscaling" | "frameGeneration"
  >;
}[] = [
  { kind: "NATIVE_HDR", label: "HDR nativo", key: "nativeHdr" },
  {
    kind: "HIGH_FIDELITY_UPSCALING",
    label: "Upscaling",
    key: "highFidelityUpscaling",
  },
  {
    kind: "FRAME_GENERATION",
    label: "Frame generation",
    key: "frameGeneration",
  },
];

const overrideOptions: readonly {
  state: GameCapabilityOverrideState;
  label: string;
}[] = [
  { state: "NO_OVERRIDE", label: "Automático" },
  { state: "FORCE_YES", label: "Sí" },
  { state: "FORCE_NO", label: "No" },
  { state: "FORCE_UNKNOWN", label: "Desconocido" },
];

export function GameCapabilitiesPanel({
  gameId,
  steamAppId,
  gogProductId = null,
  screenshotUrls = [],
  backgroundUrl = null,
}: {
  gameId: string;
  steamAppId: number | null;
  gogProductId?: string | null;
  screenshotUrls?: readonly string[];
  backgroundUrl?: string | null;
}) {
  const queryClient = useQueryClient();
  const { engine } = useNavigation();
  const queryKey = useMemo(
    () => ["game-capabilities", gameId, steamAppId, gogProductId] as const,
    [gameId, gogProductId, steamAppId],
  );
  const identity = useMemo(
    () => ({ steamAppId, gogProductId }),
    [gogProductId, steamAppId],
  );
  const query = useQuery({
    queryKey,
    queryFn: () => gameCapabilitiesService.get(gameId, identity),
    enabled: gameId.length > 0,
    staleTime: 7 * 24 * 60 * 60 * 1000,
    refetchOnWindowFocus: false,
    retry: false,
  });
  const [editingCapability, setEditingCapability] =
    useState<GameCapabilityKind | null>(null);
  const [isSavingOverride, setIsSavingOverride] = useState(false);
  const originFocusRef = useRef<string>("details-play");
  const overrideScopeId = `game-capability-override-${gameId}`;

  const closeOverride = useCallback(() => {
    const capability = editingCapability;
    if (!capability) return;
    const focusId = capabilityFocusId(capability);
    engine.requestScopeRestore(overrideScopeId, "details", focusId);
    engine.completePendingRestore("details", focusId);
    setEditingCapability(null);
  }, [editingCapability, engine, overrideScopeId]);

  const openOverride = (capability: GameCapabilityKind) => {
    originFocusRef.current =
      engine.getActiveFocusId() ?? capabilityFocusId(capability);
    engine.prepareScopeOpen(overrideScopeId, originFocusRef.current);
    setEditingCapability(capability);
  };

  const saveOverride = async (state: GameCapabilityOverrideState) => {
    if (!editingCapability || isSavingOverride) return;
    setIsSavingOverride(true);
    try {
      const next =
        state === "NO_OVERRIDE"
          ? await gameCapabilitiesService.clearOverride(
              gameId,
              editingCapability,
            )
          : await gameCapabilitiesService.setOverride(
              gameId,
              editingCapability,
              state,
            );
      queryClient.setQueryData(queryKey, next);
      closeOverride();
    } finally {
      setIsSavingOverride(false);
    }
  };

  const refresh = async () => {
    if (query.isFetching) return;
    const next = await gameCapabilitiesService.refresh(gameId, identity);
    queryClient.setQueryData(queryKey, next);
  };

  const capabilities = query.data;
  const sourceLabel = capabilities
    ? commonSourceLabel([
        capabilities.nativeHdr,
        capabilities.highFidelityUpscaling,
        capabilities.frameGeneration,
      ])
    : null;

  return (
    <section
      className="game-capabilities-panel"
      aria-labelledby="game-capabilities-heading"
    >
      <div className="game-capabilities-header">
        <div>
          <h3 id="game-capabilities-heading" className="visually-hidden">
            Capacidades del juego
          </h3>
          {sourceLabel && (
            <span className="game-capabilities-source">
              Fuente: {sourceLabel}
            </span>
          )}
        </div>
        <Focusable
          focusId="details-capabilities-refresh"
          scopeId="details"
          className="game-capabilities-refresh"
          ariaLabel="Actualizar capacidades"
          disabled={query.isFetching}
          onConfirm={() => void refresh()}
        >
          {query.isFetching ? "Actualizando…" : "Actualizar"}
        </Focusable>
      </div>
      {query.isPending && (
        <p className="game-capabilities-status">Consultando evidencia…</p>
      )}
      {query.error && (
        <p className="game-capabilities-status">
          No se pudo consultar la evidencia.
        </p>
      )}
      {capabilities && (
        <>
          <div className="game-capabilities-list">
            {capabilityRows.map(({ kind, key, label }, index) => (
              <CapabilityRow
                key={kind}
                capability={capabilities[key]}
                focusId={capabilityFocusId(kind)}
                kind={kind}
                label={label}
                gameId={gameId}
                backgroundUrl={screenshotUrls[index] ?? backgroundUrl}
                mediaType={screenshotUrls[index] ? "screenshot" : "hero"}
                onConfirm={() => openOverride(kind)}
              />
            ))}
          </div>
          <GraphicsProfilePanel gameId={gameId} capabilities={capabilities} />
        </>
      )}
      {editingCapability && capabilities && (
        <div className="game-capabilities-dialog-backdrop">
          <NavigationDialog
            scopeId={overrideScopeId}
            initialFocusId={`${overrideScopeId}-auto`}
            className="game-capabilities-override-dialog"
            onBack={() => {
              closeOverride();
              return true;
            }}
          >
            <p className="eyebrow">Modificar</p>
            <h4>
              {
                capabilityRows.find((row) => row.kind === editingCapability)
                  ?.label
              }
            </h4>
            <div className="game-capabilities-override-options">
              {overrideOptions.map(({ state, label }) => (
                <Focusable
                  key={state}
                  focusId={`${overrideScopeId}-${state === "NO_OVERRIDE" ? "auto" : state.toLowerCase()}`}
                  scopeId={overrideScopeId}
                  className="game-capabilities-override-option"
                  role="menuitem"
                  disabled={isSavingOverride}
                  onConfirm={() => void saveOverride(state)}
                >
                  {label}
                </Focusable>
              ))}
            </div>
          </NavigationDialog>
        </div>
      )}
    </section>
  );
}

function CapabilityRow({
  capability,
  focusId,
  kind,
  gameId,
  label,
  backgroundUrl,
  mediaType,
  onConfirm,
}: {
  capability: ResolvedCapability;
  focusId: string;
  kind: GameCapabilityKind;
  gameId: string;
  label: string;
  backgroundUrl?: string;
  mediaType: "hero" | "screenshot";
  onConfirm: () => void;
}) {
  return (
    <Focusable
      focusId={focusId}
      scopeId="details"
      className={`game-capability-row is-${capability.value.toLowerCase()}`}
      ariaLabel={`${label}: ${capabilityValueLabel(capability.value)}`}
      onConfirm={onConfirm}
    >
      {backgroundUrl && (
        <MediaImage
          gameId={gameId}
          mediaType={mediaType}
          reactKey={`${gameId}-capability-${label}`}
          src={backgroundUrl}
          alt=""
          className="game-capability-card-image"
          loading="eager"
          decoding="async"
          draggable={false}
        />
      )}
      <span className="game-capability-card-heading">
        <span className="game-capability-card-icon" aria-hidden="true">
          {capabilityIcon(label)}
        </span>
        <span className="game-capability-label">{label}</span>
      </span>
      <CapabilityArt kind={kind} />
      <span className="game-capability-row-copy">
        <span className="game-capability-description">
          {capabilityDescription(label, capability.value)}
        </span>
        {capability.technologies.length > 0 && (
          <span className="game-capability-technologies">
            Métodos: {capability.technologies.join(", ")}
          </span>
        )}
        <span
          className={`game-capability-state ${capabilityStateClass(capability)}`}
        >
          {capabilityStateLabel(capability)}
        </span>
      </span>
    </Focusable>
  );
}

function capabilityIcon(label: string): string {
  if (label === "HDR nativo") return "HDR";
  if (label === "Upscaling") return "U↗";
  return "FG";
}

function capabilityDescription(
  label: string,
  value: ResolvedCapability["value"],
): string {
  if (value === "UNKNOWN") return "No hay evidencia suficiente disponible.";
  if (label === "HDR nativo") {
    return value === "YES"
      ? "El juego declara soporte HDR nativo."
      : "El juego no declara soporte HDR nativo.";
  }
  if (label === "Upscaling") {
    return value === "YES"
      ? "El juego declara compatibilidad con upscaling."
      : "El juego no declara compatibilidad con upscaling.";
  }
  return value === "YES"
    ? "El juego declara compatibilidad con frame generation."
    : "El juego no declara compatibilidad con frame generation.";
}

function capabilityStateLabel(capability: ResolvedCapability): string {
  if (capability.value === "NO" && capability.alternativeAvailable === "YES") {
    return "Parcialmente compatible";
  }
  if (capability.value === "YES") return "Compatible";
  if (capability.value === "NO") return "No compatible";
  return "Desconocido";
}

function capabilityStateClass(capability: ResolvedCapability): string {
  if (capability.value === "NO" && capability.alternativeAvailable === "YES") {
    return "is-partial";
  }
  return `is-${capability.value.toLowerCase()}`;
}

function commonSourceLabel(
  capabilities: readonly ResolvedCapability[],
): string | null {
  const sources = [
    ...new Set(
      capabilities
        .map((capability) => capabilitySourceLabel(capability.source))
        .filter((source) => source !== "Sin evidencia"),
    ),
  ];
  if (sources.length === 1) return sources[0];
  if (sources.length > 1) return "Evidencia por capacidad";
  return null;
}

function CapabilityArt({ kind }: { kind: GameCapabilityKind }) {
  return (
    <span
      className={`game-capability-art is-${kind.toLowerCase()}`}
      aria-hidden="true"
    >
      <svg viewBox="0 0 100 100" fill="none">
        {kind === "NATIVE_HDR" && (
          <>
            <circle cx="50" cy="50" r="17" />
            <path d="M50 5v18M50 77v18M5 50h18M77 50h18M18 18l13 13M69 69l13 13M82 18 69 31M31 69 18 82" />
          </>
        )}
        {kind === "HIGH_FIDELITY_UPSCALING" && (
          <>
            <path d="M14 72V45h27V72H14ZM59 55V28h27v27H59Z" />
            <path d="m38 62 23-23M49 39h12v12" />
            <circle cx="24" cy="24" r="8" />
          </>
        )}
        {kind === "FRAME_GENERATION" && (
          <>
            <circle cx="50" cy="50" r="35" />
            <circle cx="50" cy="50" r="20" />
            <circle cx="50" cy="50" r="8" />
            <path d="M50 4v12M50 84v12M4 50h12M84 50h12" />
          </>
        )}
      </svg>
    </span>
  );
}

function capabilityFocusId(kind: GameCapabilityKind): string {
  return `details-capability-${kind.toLowerCase()}`;
}
