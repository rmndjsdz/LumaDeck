import { useCallback, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { NavigationDialog } from "../../ui/navigation/layouts/NavigationDialog";
import { useNavigation } from "../../ui/navigation/navigation-context";
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
    label: "High-fidelity upscaling",
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
}: {
  gameId: string;
  steamAppId: number | null;
  gogProductId?: string | null;
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

  return (
    <section
      className="game-capabilities-panel"
      aria-labelledby="game-capabilities-heading"
    >
      <div className="game-capabilities-header">
        <div>
          <p className="eyebrow">Capacidades</p>
          <h3 id="game-capabilities-heading">Conocimiento técnico</h3>
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
            {capabilityRows.map(({ kind, key, label }) => (
              <CapabilityRow
                key={kind}
                capability={capabilities[key]}
                focusId={capabilityFocusId(kind)}
                label={label}
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
  label,
  onConfirm,
}: {
  capability: ResolvedCapability;
  focusId: string;
  label: string;
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
      <span className="game-capability-row-copy">
        <span className="game-capability-label">{label}</span>
        {capability.technologies.length > 0 && (
          <span className="game-capability-technologies">
            {capability.technologies.join(" · ")}
          </span>
        )}
        <span className="game-capability-source">
          {capabilitySourceLabel(capability.source)}
          {capability.stale ? " · Offline" : ""}
          {capability.hasConflict ? " · Override contradice evidencia" : ""}
        </span>
      </span>
      <strong className="game-capability-value">
        {capabilityValueLabel(capability.value)}
      </strong>
    </Focusable>
  );
}

function capabilityFocusId(kind: GameCapabilityKind): string {
  return `details-capability-${kind.toLowerCase()}`;
}
