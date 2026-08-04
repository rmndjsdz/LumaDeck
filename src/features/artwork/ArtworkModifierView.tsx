import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { FocusScope } from "../../ui/navigation/focus/FocusScope";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import { NavigationTabs } from "../../ui/navigation/layouts/NavigationTabs";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { useNavigationStore } from "../../stores/navigation-store";
import { artworkErrorMessage, artworkService } from "./artwork-service";
import {
  ARTWORK_FILTERS,
  ARTWORK_SLOTS,
  artworkFilterLabel,
  artworkSlotLabel,
  type ArtworkFilterKind,
  type ArtworkPreviewCandidate,
  type ArtworkSlot,
} from "./artwork-types";
import {
  ARTWORK_ACTION_REGION_ID,
  ARTWORK_APPLY_FOCUS_ID,
  ARTWORK_CANDIDATE_REGION_ID,
  ARTWORK_EMPTY_RETRY_FOCUS_ID,
  ARTWORK_FILTER_REGION_ID,
  ARTWORK_RESTORE_FOCUS_ID,
  ARTWORK_RETRY_FOCUS_ID,
  ARTWORK_SCOPE_ID,
  ARTWORK_SLOT_REGION_ID,
  actionNavigation,
  artworkFallbackFocusId,
  candidateNavigation,
  filterFocusId,
  filterNavigation,
  isArtworkGridSlot,
  slotFocusId,
  slotNavigation,
  type ArtworkCandidateGroupNavigation,
  type ArtworkNavigationState,
} from "./artwork-navigation";

const EMPTY_CANDIDATES: ArtworkPreviewCandidate[] = [];

type ArtworkCandidateGroup = {
  key: string;
  width: number;
  height: number;
  candidates: ArtworkPreviewCandidate[];
};

function groupCandidatesBySize(
  candidates: ArtworkPreviewCandidate[],
): ArtworkCandidateGroup[] {
  const groups = new Map<string, ArtworkCandidateGroup>();
  for (const candidate of candidates) {
    const key = `${candidate.width}x${candidate.height}`;
    const group = groups.get(key);
    if (group) {
      group.candidates.push(candidate);
      continue;
    }
    groups.set(key, {
      key,
      width: candidate.width,
      height: candidate.height,
      candidates: [candidate],
    });
  }
  const sortedGroups = [...groups.values()].sort(
    (left, right) =>
      right.width * right.height - left.width * left.height ||
      right.width - left.width ||
      right.height - left.height,
  );
  return sortedGroups;
}

export function ArtworkModifierView({
  game,
  onClose,
}: {
  game: Game;
  onClose: () => void;
}) {
  const { engine } = useNavigation();
  const activeFocusId = useNavigationStore((state) => state.activeFocusId);
  const queryClient = useQueryClient();
  const [slot, setSlot] = useState<ArtworkSlot>("grid_horizontal");
  const [styleFilter, setStyleFilter] = useState<ArtworkFilterKind>("all");
  const [selectedCandidateId, setSelectedCandidateId] = useState<string | null>(
    null,
  );
  const lastCandidateFocusIdRef = useRef<string | undefined>(undefined);
  const close = () => {
    void artworkService.cancel();
    onClose();
  };
  const query = useQuery({
    queryKey: ["steamgriddb-artwork", game.id, slot, styleFilter],
    queryFn: () =>
      artworkService.search({ gameId: game.id, slot, styleFilter }),
    staleTime: 5 * 60 * 1000,
    gcTime: 10 * 60 * 1000,
    retry: false,
  });
  const currentArtworkQuery = useQuery({
    queryKey: ["steamgriddb-current-artwork", game.id, slot],
    queryFn: () => artworkService.current(game.id, slot),
    staleTime: 30_000,
    retry: false,
  });
  const applyMutation = useMutation({
    mutationFn: (candidateId: string) =>
      artworkService.apply({
        gameId: game.id,
        slot,
        styleFilter,
        candidateId,
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["games"] });
      await queryClient.invalidateQueries({
        queryKey: ["steamgriddb-current-artwork", game.id, slot],
      });
    },
  });
  const restoreMutation = useMutation({
    mutationFn: () => artworkService.restore(game.id, slot),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["games"] });
      await queryClient.invalidateQueries({
        queryKey: ["steamgriddb-current-artwork", game.id, slot],
      });
    },
  });
  const candidates = query.data?.candidates ?? EMPTY_CANDIDATES;
  const candidateGroups = useMemo(
    () => groupCandidatesBySize(candidates),
    [candidates],
  );
  const displayCandidates = useMemo(
    () => candidateGroups.flatMap((group) => group.candidates),
    [candidateGroups],
  );
  const currentArtworkUrl =
    currentArtworkQuery.data ?? currentArtworkForSlot(game, slot);
  const focusedCandidate = useMemo(
    () =>
      displayCandidates.find(
        (candidate) =>
          `artwork-candidate-${candidate.candidateId}` === activeFocusId,
      ) ??
      displayCandidates.find(
        (candidate) => candidate.candidateId === selectedCandidateId,
      ) ??
      displayCandidates[0],
    [activeFocusId, displayCandidates, selectedCandidateId],
  );

  const changeSlot = (nextSlot: ArtworkSlot) => {
    void artworkService.cancel();
    setSlot(nextSlot);
    setSelectedCandidateId(null);
  };

  const changeFilter = (nextFilter: ArtworkFilterKind) => {
    void artworkService.cancel();
    setStyleFilter(nextFilter);
    setSelectedCandidateId(null);
  };

  const isBusy = applyMutation.isPending || restoreMutation.isPending;
  const gridSlot = isArtworkGridSlot(slot);
  const queryState: ArtworkNavigationState["queryState"] = query.isPending
    ? "loading"
    : query.isError
      ? "error"
      : candidates.length === 0
        ? "empty"
        : "ready";
  const firstCandidateFocusId = displayCandidates[0]
    ? `artwork-candidate-${displayCandidates[0].candidateId}`
    : undefined;
  if (
    activeFocusId?.startsWith("artwork-candidate-") &&
    candidates.some(
      (candidate) =>
        `artwork-candidate-${candidate.candidateId}` === activeFocusId,
    )
  ) {
    lastCandidateFocusIdRef.current = activeFocusId;
  }
  const navigationState: ArtworkNavigationState = {
    slot,
    filter: styleFilter,
    candidateCount: candidates.length,
    firstCandidateFocusId,
    queryState,
    applyAvailable: Boolean(selectedCandidateId) && !isBusy,
    restoreAvailable: !isBusy,
    lastCandidateFocusId: lastCandidateFocusIdRef.current,
  };
  const activeArtworkFocusId =
    activeFocusId &&
    engine.registry.get(activeFocusId)?.scopeId === ARTWORK_SCOPE_ID
      ? activeFocusId
      : null;
  const fallbackFocusId = artworkFallbackFocusId(
    activeArtworkFocusId,
    navigationState,
  );

  useLayoutEffect(() => {
    if (useNavigationStore.getState().activeScopeId !== ARTWORK_SCOPE_ID) {
      return;
    }
    const activeEntry = activeFocusId
      ? engine.registry.get(activeFocusId)
      : undefined;
    const activeCandidateStillExists =
      activeFocusId?.startsWith("artwork-candidate-") &&
      candidates.some(
        (candidate) =>
          `artwork-candidate-${candidate.candidateId}` === activeFocusId,
      );
    const activeActionIsAvailable =
      activeFocusId === ARTWORK_APPLY_FOCUS_ID
        ? navigationState.applyAvailable
        : activeFocusId === ARTWORK_RESTORE_FOCUS_ID
          ? navigationState.restoreAvailable
          : true;
    if (
      activeEntry &&
      !activeEntry.disabled &&
      !activeEntry.hidden &&
      activeEntry.element.isConnected &&
      (!activeFocusId?.startsWith("artwork-candidate-") ||
        activeCandidateStillExists) &&
      activeActionIsAvailable
    ) {
      return;
    }
    if (engine.registry.get(fallbackFocusId)) {
      engine.focus(fallbackFocusId);
    }
  }, [
    activeFocusId,
    candidates,
    engine,
    fallbackFocusId,
    navigationState.applyAvailable,
    navigationState.restoreAvailable,
  ]);

  return (
    <FocusScope
      scopeId={ARTWORK_SCOPE_ID}
      parentScopeId="details"
      initialFocusId={slotFocusId("grid_horizontal")}
      restoreFocus
      rememberScroll
      trapFocus
      modal
      activateOnMount
      onBack={close}
    >
      <section className="artwork-modifier" aria-label="Selector de arte">
        <div className="artwork-modifier-layout">
          <div className="artwork-modifier-main">
            <div className="artwork-section-heading">
              <div>
                <p className="eyebrow">Destino</p>
                <h3>Selecciona un slot</h3>
              </div>
              <span className="artwork-query-status">
                {query.isFetching
                  ? "Consultando…"
                  : `${candidates.length} resultados`}
              </span>
            </div>
            <NavigationTabs
              groupId={ARTWORK_SLOT_REGION_ID}
              selectedId={slotFocusId(slot)}
              className="artwork-slot-grid"
              navigationRegion={{
                regionId: ARTWORK_SLOT_REGION_ID,
              }}
            >
              {ARTWORK_SLOTS.map((candidateSlot) => (
                <Focusable
                  key={candidateSlot}
                  focusId={slotFocusId(candidateSlot)}
                  scopeId={ARTWORK_SCOPE_ID}
                  className={`artwork-slot-button ${slot === candidateSlot ? "is-active" : ""}`}
                  ariaLabel={artworkSlotLabel(candidateSlot)}
                  ariaSelected={slot === candidateSlot}
                  navigation={slotNavigation(navigationState)}
                  onConfirm={() => changeSlot(candidateSlot)}
                >
                  {artworkSlotLabel(candidateSlot)}
                </Focusable>
              ))}
            </NavigationTabs>

            {(slot === "grid_horizontal" ||
              slot === "grid_vertical" ||
              slot === "grid_square") && (
              <NavigationTabs
                groupId={ARTWORK_FILTER_REGION_ID}
                selectedId={filterFocusId(styleFilter)}
                className="artwork-filter-grid"
                navigationRegion={{
                  regionId: ARTWORK_FILTER_REGION_ID,
                  parentRegionId: ARTWORK_SLOT_REGION_ID,
                  exitFocusId: slotFocusId(slot),
                }}
              >
                {ARTWORK_FILTERS.map((candidateFilter) => (
                  <Focusable
                    key={candidateFilter}
                    focusId={filterFocusId(candidateFilter)}
                    scopeId={ARTWORK_SCOPE_ID}
                    className={`artwork-filter-button ${styleFilter === candidateFilter ? "is-active" : ""}`}
                    ariaLabel={artworkFilterLabel(candidateFilter)}
                    ariaSelected={styleFilter === candidateFilter}
                    navigation={filterNavigation(navigationState)}
                    onConfirm={() => changeFilter(candidateFilter)}
                  >
                    {artworkFilterLabel(candidateFilter)}
                  </Focusable>
                ))}
              </NavigationTabs>
            )}

            {query.isPending && <ArtworkLoadingState />}
            {query.isError && (
              <div className="artwork-state artwork-state-error">
                <strong>{artworkErrorMessage(query.error)}</strong>
                <Focusable
                  focusId={ARTWORK_RETRY_FOCUS_ID}
                  scopeId={ARTWORK_SCOPE_ID}
                  className="settings-button primary"
                  onConfirm={() => void query.refetch()}
                >
                  Reintentar
                </Focusable>
              </div>
            )}
            {!query.isPending && !query.isError && candidates.length === 0 && (
              <div className="artwork-state">
                <strong>No hay arte disponible para este filtro.</strong>
                <span>Prueba otro slot o estilo.</span>
                <Focusable
                  focusId={ARTWORK_EMPTY_RETRY_FOCUS_ID}
                  scopeId={ARTWORK_SCOPE_ID}
                  className="settings-button"
                  onConfirm={() => void query.refetch()}
                >
                  Consultar de nuevo
                </Focusable>
              </div>
            )}
            {candidates.length > 0 && (
              <div className="artwork-candidate-grid">
                {candidateGroups.map((group, groupIndex) => {
                  const previousGroup = candidateGroups[groupIndex - 1];
                  const nextGroup = candidateGroups[groupIndex + 1];
                  return (
                    <div className="artwork-candidate-group" key={group.key}>
                      <div
                        className="artwork-size-group-label"
                        aria-hidden="true"
                      >
                        {group.width} × {group.height}
                      </div>
                      <NavigationGrid
                        groupId={`${ARTWORK_CANDIDATE_REGION_ID}-${group.key}`}
                        columns={4}
                        itemCount={group.candidates.length}
                        className="artwork-candidate-group-grid"
                        regionId={ARTWORK_CANDIDATE_REGION_ID}
                        parentRegionId={
                          gridSlot
                            ? ARTWORK_FILTER_REGION_ID
                            : ARTWORK_SLOT_REGION_ID
                        }
                        exitFocusId={
                          gridSlot
                            ? filterFocusId(styleFilter)
                            : slotFocusId(slot)
                        }
                      >
                        {group.candidates.map((candidate, indexInGroup) => {
                          const groupNavigation: ArtworkCandidateGroupNavigation =
                            {
                              groupIndex,
                              groupCount: candidateGroups.length,
                              indexInGroup,
                              groupSize: group.candidates.length,
                              previousGroupFocusIds:
                                previousGroup?.candidates.map(
                                  (previousCandidate) =>
                                    `artwork-candidate-${previousCandidate.candidateId}`,
                                ),
                              nextGroupFocusIds: nextGroup?.candidates.map(
                                (nextCandidate) =>
                                  `artwork-candidate-${nextCandidate.candidateId}`,
                              ),
                            };
                          return (
                            <ArtworkCandidateCard
                              key={candidate.candidateId}
                              candidate={candidate}
                              index={indexInGroup}
                              groupNavigation={groupNavigation}
                              navigationState={navigationState}
                              selected={
                                candidate.candidateId === selectedCandidateId
                              }
                              onSelect={() =>
                                setSelectedCandidateId(candidate.candidateId)
                              }
                            />
                          );
                        })}
                      </NavigationGrid>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          <aside
            className="artwork-modifier-aside"
            aria-label="Detalle del arte"
          >
            <p className="eyebrow">Vista previa actual</p>
            <div className="artwork-current-preview">
              {currentArtworkUrl ? (
                <img src={currentArtworkUrl} alt="" draggable={false} />
              ) : (
                <span className="artwork-current-empty">
                  {currentArtworkQuery.isPending
                    ? "Consultando arte actual…"
                    : "Sin arte aplicado para este slot"}
                </span>
              )}
            </div>
            <p className="eyebrow">Candidato enfocado</p>
            {focusedCandidate ? (
              <ArtworkCandidateDetails candidate={focusedCandidate} />
            ) : (
              <p className="artwork-empty-detail">
                Navega por los resultados para ver sus detalles.
              </p>
            )}
            <Focusable
              focusId={ARTWORK_APPLY_FOCUS_ID}
              scopeId={ARTWORK_SCOPE_ID}
              navigationRegion={{
                regionId: ARTWORK_ACTION_REGION_ID,
                parentRegionId: ARTWORK_CANDIDATE_REGION_ID,
                exitFocusId:
                  navigationState.lastCandidateFocusId ??
                  firstCandidateFocusId ??
                  (gridSlot ? filterFocusId(styleFilter) : slotFocusId(slot)),
              }}
              navigation={actionNavigation("apply", navigationState)}
              className="settings-button primary"
              disabled={!selectedCandidateId || isBusy}
              onConfirm={() => {
                if (selectedCandidateId)
                  applyMutation.mutate(selectedCandidateId);
              }}
            >
              {applyMutation.isPending ? "Aplicando…" : "Aplicar arte"}
            </Focusable>
            <Focusable
              focusId={ARTWORK_RESTORE_FOCUS_ID}
              scopeId={ARTWORK_SCOPE_ID}
              navigationRegion={{
                regionId: ARTWORK_ACTION_REGION_ID,
                parentRegionId: ARTWORK_CANDIDATE_REGION_ID,
                exitFocusId:
                  navigationState.lastCandidateFocusId ??
                  firstCandidateFocusId ??
                  (gridSlot ? filterFocusId(styleFilter) : slotFocusId(slot)),
              }}
              navigation={actionNavigation("restore", navigationState)}
              className="settings-button"
              disabled={isBusy}
              onConfirm={() => restoreMutation.mutate()}
            >
              {restoreMutation.isPending
                ? "Restaurando…"
                : "Restaurar original"}
            </Focusable>
            {(applyMutation.isError || restoreMutation.isError) && (
              <p className="artwork-feedback is-error" aria-live="polite">
                {artworkErrorMessage(
                  applyMutation.error ?? restoreMutation.error,
                )}
              </p>
            )}
            {applyMutation.isSuccess && (
              <p className="artwork-feedback is-success" aria-live="polite">
                Arte aplicado localmente.
              </p>
            )}
          </aside>
        </div>
      </section>
    </FocusScope>
  );
}

function ArtworkCandidateCard({
  candidate,
  index,
  groupNavigation,
  navigationState,
  selected,
  onSelect,
}: {
  candidate: ArtworkPreviewCandidate;
  index: number;
  groupNavigation: ArtworkCandidateGroupNavigation;
  navigationState: ArtworkNavigationState;
  selected: boolean;
  onSelect: () => void;
}) {
  const [thumbnailFailed, setThumbnailFailed] = useState(false);
  return (
    <Focusable
      focusId={`artwork-candidate-${candidate.candidateId}`}
      scopeId={ARTWORK_SCOPE_ID}
      gridIndex={index}
      className={`artwork-candidate ${selected ? "is-selected" : ""}`}
      ariaLabel={`Arte ${candidate.width} por ${candidate.height}`}
      navigation={candidateNavigation(index, navigationState, groupNavigation)}
      onConfirm={onSelect}
    >
      <span
        className="artwork-candidate-image"
        style={{ aspectRatio: `${candidate.width} / ${candidate.height}` }}
      >
        {thumbnailFailed ? (
          <span className="artwork-thumbnail-placeholder">
            Sin vista previa
          </span>
        ) : (
          <img
            src={candidate.thumbnailUrl}
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={() => setThumbnailFailed(true)}
          />
        )}
      </span>
      <span className="artwork-candidate-meta">
        <strong>{candidate.gridStyle ?? "Estándar"}</strong>
        <small>
          {candidate.width} × {candidate.height}
        </small>
      </span>
    </Focusable>
  );
}

function ArtworkCandidateDetails({
  candidate,
}: {
  candidate: ArtworkPreviewCandidate;
}) {
  return (
    <div className="artwork-candidate-details">
      <strong>
        {candidate.width} × {candidate.height}
      </strong>
      <span>Estilo: {candidate.gridStyle ?? "Estándar"}</span>
      <span>Formato: {candidate.mimeType ?? "Imagen"}</span>
      {candidate.authorName && <span>Autor: {candidate.authorName}</span>}
      {candidate.score !== null && (
        <span>Valoración: {candidate.score.toFixed(1)}</span>
      )}
      <small>La imagen original solo se descargará al confirmar.</small>
    </div>
  );
}

function ArtworkLoadingState() {
  return (
    <div className="artwork-state" aria-live="polite">
      <strong>Consultando SteamGridDB…</strong>
      <span>Las miniaturas se cargarán progresivamente.</span>
    </div>
  );
}

function currentArtworkForSlot(game: Game, slot: ArtworkSlot): string {
  switch (slot) {
    case "grid_horizontal":
      return game.coverUrl;
    case "grid_vertical":
      return game.verticalCoverUrl || game.coverUrl;
    case "hero":
      return game.backgroundUrl;
    case "logo":
      return game.logoUrl || game.coverUrl;
    case "grid_square":
    case "icon":
      return "";
  }
}
