import { describe, expect, it } from "vitest";
import {
  actionNavigation,
  artworkFallbackFocusId,
  artworkResultEntryFocusId,
  candidateNavigation,
  filterNavigation,
  slotNavigation,
  type ArtworkNavigationState,
} from "./artwork-navigation";

function state(
  overrides: Partial<ArtworkNavigationState> = {},
): ArtworkNavigationState {
  return {
    slot: "grid_horizontal",
    filter: "all",
    candidateCount: 8,
    firstCandidateFocusId: "artwork-candidate-first",
    queryState: "ready",
    applyAvailable: false,
    restoreAvailable: true,
    ...overrides,
  };
}

describe("Artwork navigation contract", () => {
  it("moves from slots to the active filter and keeps slots linear", () => {
    expect(slotNavigation(state())).toMatchObject({
      down: "artwork-filter-all",
    });
    expect(slotNavigation(state()).up).toBeUndefined();
  });

  it("moves from filters to the first candidate", () => {
    expect(filterNavigation(state())).toMatchObject({
      up: "artwork-slot-grid_horizontal",
      down: "artwork-candidate-first",
    });
  });

  it("moves from the first row to the active filter", () => {
    expect(candidateNavigation(0, state())).toMatchObject({
      up: "artwork-filter-all",
    });
  });

  it("moves from the last column to a valid action explicitly", () => {
    expect(candidateNavigation(3, state())).toMatchObject({
      right: "artwork-restore",
    });
  });

  it("returns from actions to the previously focused candidate", () => {
    expect(
      actionNavigation(
        "restore",
        state({ lastCandidateFocusId: "artwork-candidate-7" }),
      ),
    ).toMatchObject({ left: "artwork-candidate-7" });
  });

  it("uses a deterministic fallback when a slot has fewer results", () => {
    expect(candidateNavigation(0, state({ candidateCount: 1 }))).toMatchObject({
      up: "artwork-filter-all",
      down: "artwork-restore",
    });
  });

  it("falls back when the previous candidate no longer exists", () => {
    expect(
      artworkFallbackFocusId(
        "artwork-candidate-previous",
        state({ candidateCount: 2 }),
      ),
    ).toBe("artwork-candidate-first");
  });

  it("enters the empty state action instead of an absent grid", () => {
    const empty = state({
      candidateCount: 0,
      firstCandidateFocusId: undefined,
      queryState: "empty",
    });
    expect(artworkResultEntryFocusId(empty)).toBe("artwork-empty-retry");
    expect(filterNavigation(empty).down).toBe("artwork-empty-retry");
  });

  it("enters retry on error", () => {
    const error = state({
      candidateCount: 0,
      firstCandidateFocusId: undefined,
      queryState: "error",
    });
    expect(filterNavigation(error).down).toBe("artwork-retry");
  });

  it("does not invent a destination while loading", () => {
    const loading = state({
      candidateCount: 0,
      firstCandidateFocusId: undefined,
      queryState: "loading",
    });
    expect(filterNavigation(loading).down).toBeUndefined();
    expect(artworkFallbackFocusId(null, loading)).toBe(
      "artwork-slot-grid_horizontal",
    );
  });

  it("replaces the entry deterministically when results arrive", () => {
    const loading = state({
      candidateCount: 0,
      firstCandidateFocusId: undefined,
      queryState: "loading",
    });
    const ready = state({
      candidateCount: 2,
      firstCandidateFocusId: "artwork-candidate-new",
    });
    expect(filterNavigation(loading).down).toBeUndefined();
    expect(filterNavigation(ready).down).toBe("artwork-candidate-new");
  });

  it("restores the slot as the parent for non-grid slots", () => {
    const nonGrid = state({ slot: "hero" });
    expect(candidateNavigation(0, nonGrid).up).toBe("artwork-slot-hero");
    expect(filterNavigation(nonGrid).up).toBe("artwork-slot-hero");
  });

  it("never returns a null fallback", () => {
    expect(
      artworkFallbackFocusId(
        null,
        state({
          candidateCount: 0,
          firstCandidateFocusId: undefined,
          queryState: "loading",
        }),
      ),
    ).toBeTruthy();
  });

  it("does not use geometry to resolve any boundary", () => {
    const first = candidateNavigation(0, state());
    const last = candidateNavigation(7, state());
    expect(first.up).toBe("artwork-filter-all");
    expect(last.right).toBe("artwork-restore");
  });

  it("enters the next rendered size group at its first card", () => {
    const grouped = {
      groupIndex: 1,
      groupCount: 3,
      indexInGroup: 0,
      groupSize: 1,
      previousGroupFocusIds: [
        "artwork-candidate-0",
        "artwork-candidate-1",
        "artwork-candidate-2",
        "artwork-candidate-3",
      ],
      nextGroupFocusIds: [
        "artwork-candidate-5",
        "artwork-candidate-6",
        "artwork-candidate-7",
      ],
    };

    expect(candidateNavigation(0, state(), grouped)).toMatchObject({
      up: "artwork-candidate-0",
      down: "artwork-candidate-5",
    });
  });

  it("does not flatten an incomplete group into the next group's columns", () => {
    const grouped = {
      groupIndex: 2,
      groupCount: 3,
      indexInGroup: 0,
      groupSize: 5,
      previousGroupFocusIds: ["artwork-candidate-4"],
    };

    expect(candidateNavigation(0, state(), grouped).up).toBe(
      "artwork-candidate-4",
    );
  });
});
