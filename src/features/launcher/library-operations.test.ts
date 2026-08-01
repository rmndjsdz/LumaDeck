import { describe, expect, it } from "vitest";
import { createMockCatalog } from "../catalog/mock-catalog";
import { filterAndSortGames } from "./library-operations";

describe("library operations", () => {
  it("filters by title and sorts by time without mutating the catalog", () => {
    const games = createMockCatalog();
    const filtered = filterAndSortGames(games, "Aether", "all", "time");

    expect(filtered.length).toBeGreaterThan(0);
    expect(filtered.every((game) => game.title.includes("Aether"))).toBe(true);
    expect(filtered[0].playtimeMinutes).toBeGreaterThanOrEqual(
      filtered[filtered.length - 1]?.playtimeMinutes ?? 0,
    );
    expect(games).toHaveLength(200);
  });
});
