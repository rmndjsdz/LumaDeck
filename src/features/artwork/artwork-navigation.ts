import type { FocusNavigationOverrides } from "../../ui/navigation/core/navigation-types";
import type { ArtworkFilterKind, ArtworkSlot } from "./artwork-types";

export const ARTWORK_SCOPE_ID = "artwork-modifier";
export const ARTWORK_SLOT_REGION_ID = "artwork-slots";
export const ARTWORK_FILTER_REGION_ID = "artwork-filters";
export const ARTWORK_CANDIDATE_REGION_ID = "artwork-candidates";
export const ARTWORK_ACTION_REGION_ID = "artwork-actions";

export const ARTWORK_RETRY_FOCUS_ID = "artwork-retry";
export const ARTWORK_EMPTY_RETRY_FOCUS_ID = "artwork-empty-retry";
export const ARTWORK_APPLY_FOCUS_ID = "artwork-apply";
export const ARTWORK_RESTORE_FOCUS_ID = "artwork-restore";

export type ArtworkQueryState = "loading" | "error" | "empty" | "ready";

export type ArtworkNavigationState = {
  slot: ArtworkSlot;
  filter: ArtworkFilterKind;
  candidateCount: number;
  firstCandidateFocusId?: string;
  queryState: ArtworkQueryState;
  applyAvailable: boolean;
  restoreAvailable: boolean;
  lastCandidateFocusId?: string;
};

export type ArtworkCandidateGroupNavigation = {
  groupIndex: number;
  groupCount: number;
  indexInGroup: number;
  groupSize: number;
  previousGroupFocusIds?: readonly string[];
  nextGroupFocusIds?: readonly string[];
};

export function isArtworkGridSlot(slot: ArtworkSlot): boolean {
  return (
    slot === "grid_horizontal" ||
    slot === "grid_vertical" ||
    slot === "grid_square"
  );
}

export function slotFocusId(slot: ArtworkSlot): string {
  return `artwork-slot-${slot}`;
}

export function filterFocusId(filter: ArtworkFilterKind): string {
  return `artwork-filter-${filter}`;
}

export function artworkResultEntryFocusId(
  state: ArtworkNavigationState,
): string | undefined {
  if (state.candidateCount > 0 && state.firstCandidateFocusId) {
    return state.firstCandidateFocusId;
  }
  if (state.queryState === "error") return ARTWORK_RETRY_FOCUS_ID;
  if (state.queryState === "empty") return ARTWORK_EMPTY_RETRY_FOCUS_ID;
  return undefined;
}

export function artworkActionTargetFocusId(
  state: ArtworkNavigationState,
): string | undefined {
  if (state.applyAvailable) return ARTWORK_APPLY_FOCUS_ID;
  if (state.restoreAvailable) return ARTWORK_RESTORE_FOCUS_ID;
  return undefined;
}

export function slotNavigation(
  state: ArtworkNavigationState,
): FocusNavigationOverrides {
  const down = isArtworkGridSlot(state.slot)
    ? filterFocusId(state.filter)
    : artworkResultEntryFocusId(state);
  return {
    ...(down ? { down } : {}),
  };
}

export function filterNavigation(
  state: ArtworkNavigationState,
): FocusNavigationOverrides {
  const down = artworkResultEntryFocusId(state);
  return {
    up: slotFocusId(state.slot),
    ...(down ? { down } : {}),
  };
}

export function candidateNavigation(
  index: number,
  state: ArtworkNavigationState,
  group: ArtworkCandidateGroupNavigation = {
    groupIndex: 0,
    groupCount: 1,
    indexInGroup: index,
    groupSize: state.candidateCount,
  },
): FocusNavigationOverrides {
  const overrides: FocusNavigationOverrides = {};
  const firstRow = group.indexInGroup < 4;
  const lastRow = group.indexInGroup + 4 >= group.groupSize;
  const lastColumn = group.indexInGroup % 4 === 3;
  const parentFocusId = isArtworkGridSlot(state.slot)
    ? filterFocusId(state.filter)
    : slotFocusId(state.slot);
  const actionFocusId = artworkActionTargetFocusId(state);

  if (firstRow) {
    const previousGroupSize = group.previousGroupFocusIds?.length ?? 0;
    const previousGroupLastRowStart =
      Math.max(0, Math.ceil(previousGroupSize / 4) - 1) * 4;
    const previousGroupFocusId =
      group.previousGroupFocusIds?.[
        Math.min(
          previousGroupSize - 1,
          previousGroupLastRowStart + (group.indexInGroup % 4),
        )
      ];
    overrides.up = previousGroupFocusId ?? parentFocusId;
  }
  if (lastRow) {
    const nextGroupFocusIds = group.nextGroupFocusIds;
    if (nextGroupFocusIds?.length) {
      overrides.down =
        nextGroupFocusIds[
          Math.min(group.indexInGroup % 4, nextGroupFocusIds.length - 1)
        ];
    } else if (actionFocusId) {
      overrides.down = actionFocusId;
    }
  }
  if (lastColumn && actionFocusId) overrides.right = actionFocusId;
  return overrides;
}

export type ArtworkAction = "apply" | "restore";

export function actionNavigation(
  action: ArtworkAction,
  state: ArtworkNavigationState,
): FocusNavigationOverrides {
  const fallbackFocusId =
    state.lastCandidateFocusId ??
    (isArtworkGridSlot(state.slot)
      ? filterFocusId(state.filter)
      : slotFocusId(state.slot));
  const otherAction =
    action === "apply" && state.restoreAvailable
      ? ARTWORK_RESTORE_FOCUS_ID
      : action === "restore" && state.applyAvailable
        ? ARTWORK_APPLY_FOCUS_ID
        : undefined;
  return {
    left: fallbackFocusId,
    ...(otherAction ? { up: otherAction, down: otherAction } : {}),
  };
}

export function artworkFallbackFocusId(
  activeFocusId: string | null,
  state: ArtworkNavigationState,
): string {
  if (!activeFocusId) return slotFocusId(state.slot);

  const resultEntry = artworkResultEntryFocusId(state);
  const firstCandidate = state.firstCandidateFocusId;
  const candidateStillExists =
    Boolean(firstCandidate) && state.candidateCount > 0;
  const activeCandidate = activeFocusId?.startsWith("artwork-candidate-");
  const activeAction =
    activeFocusId === ARTWORK_APPLY_FOCUS_ID ||
    activeFocusId === ARTWORK_RESTORE_FOCUS_ID;

  if (activeCandidate && candidateStillExists && firstCandidate) {
    return firstCandidate;
  }
  if (activeAction && resultEntry) return resultEntry;
  if (isArtworkGridSlot(state.slot)) return filterFocusId(state.filter);
  return slotFocusId(state.slot);
}
