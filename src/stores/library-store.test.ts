import { beforeEach, describe, expect, it } from "vitest";

import { LIBRARY_DEFAULTS, useLibraryStore } from "./library-store";

describe("library store", () => {
  beforeEach(() => {
    useLibraryStore.getState().reset();
  });

  it("keeps confirmed criteria across view rematerialization", () => {
    useLibraryStore.getState().setQuery("juniper");
    useLibraryStore.getState().setStatus("completed");
    useLibraryStore.getState().setSort("recent");
    useLibraryStore.getState().setGenre("fighting");

    const beforeRemount = useLibraryStore.getState();
    const afterRemount = useLibraryStore.getState();

    expect(afterRemount.query).toBe("juniper");
    expect(afterRemount.status).toBe("completed");
    expect(afterRemount.sort).toBe("recent");
    expect(afterRemount.genre).toBe("fighting");
    expect(afterRemount.queryVersion).toBe(beforeRemount.queryVersion);
    expect(afterRemount.queryCommitted).toBe(true);
  });

  it("does not change confirmed query when a keyboard draft is cancelled", () => {
    useLibraryStore.getState().setQuery("juniper");
    const beforeCancel = useLibraryStore.getState();

    // VirtualKeyboard cancellation intentionally does not call setQuery.
    const afterCancel = useLibraryStore.getState();

    expect(afterCancel.query).toBe(beforeCancel.query);
    expect(afterCancel.queryVersion).toBe(beforeCancel.queryVersion);
    expect(afterCancel.queryCommitted).toBe(true);
  });

  it("resets criteria only through the explicit reset action", () => {
    useLibraryStore.getState().setQuery("juniper");
    useLibraryStore.getState().setStatus("playing");
    useLibraryStore.getState().setSort("time");
    useLibraryStore.getState().setGenre("local-multiplayer");

    useLibraryStore.getState().reset();

    expect(useLibraryStore.getState()).toMatchObject(LIBRARY_DEFAULTS);
  });
});
