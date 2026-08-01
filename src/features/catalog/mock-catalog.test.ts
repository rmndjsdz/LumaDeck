import { describe, expect, it } from "vitest";
import { createMockCatalog } from "./mock-catalog";

describe("mock catalog", () => {
  it("creates exactly 200 deterministic local games", () => {
    const first = createMockCatalog();
    const second = createMockCatalog();

    expect(first).toHaveLength(200);
    expect(first).toEqual(second);
    expect(new Set(first.map((game) => game.id)).size).toBe(200);
    expect(
      first.every((game) => game.coverUrl.startsWith("data:image/svg+xml")),
    ).toBe(true);
    expect(
      first.every((game) =>
        game.backgroundUrl.startsWith("data:image/svg+xml"),
      ),
    ).toBe(true);
  });
});
