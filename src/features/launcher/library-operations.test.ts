import { describe, expect, it } from "vitest";
import { createMockCatalog } from "../catalog/mock-catalog";
import {
  filterAndSortGames,
  getLibraryGenreIds,
  matchesLibraryGenre,
  normalizeLibraryGenre,
} from "./library-operations";

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

  it("does not return hidden games for normal library search", () => {
    const games = createMockCatalog();
    const hidden = games[0];
    if (!hidden) throw new Error("expected mock game");
    hidden.hidden = true;

    const filtered = filterAndSortGames(games, hidden.title, "all", "title");

    expect(filtered).toHaveLength(0);
  });

  it("normalizes genre aliases and explicit local multiplayer metadata", () => {
    const source = createMockCatalog()[0];
    if (!source) throw new Error("expected mock game");
    const game = {
      ...source,
      genres: ["Beat 'em up", "Local Multiplayer"],
    };

    expect(normalizeLibraryGenre("Beat 'em up")).toBe("beatemup");
    expect(getLibraryGenreIds(game)).toEqual([
      "beat-em-up",
      "local-multiplayer",
    ]);

    expect(
      getLibraryGenreIds({
        ...source,
        genres: ["Open World", "Sandbox"],
      }),
    ).toEqual(["open-world", "sandbox"]);

    expect(getLibraryGenreIds({ ...source, genres: ["3D Fighter"] })).toEqual([
      "fighting",
    ]);

    expect(
      getLibraryGenreIds({
        ...source,
        title: "Streets of Rage 4",
        genres: ["Action"],
      }),
    ).toContain("beat-em-up");

    expect(
      getLibraryGenreIds({
        ...source,
        title: "MARVEL Cosmic Invasion",
        genres: ["Action", "Adventure", "Casual", "Indie"],
      }),
    ).toContain("beat-em-up");

    expect(
      getLibraryGenreIds({
        ...source,
        title: "Battletoads",
        genres: ["Action"],
      }),
    ).toContain("beat-em-up");

    expect(
      getLibraryGenreIds({
        ...source,
        title: "GUILTY GEAR -STRIVE-",
        genres: ["Action"],
      }),
    ).toContain("fighting");

    expect(
      getLibraryGenreIds({
        ...source,
        title: "Resident Evil 4",
        genres: ["Action", "Adventure"],
      }),
    ).toContain("horror");

    expect(
      getLibraryGenreIds({
        ...source,
        title: "SILENT HILL 2",
        genres: ["Action", "Adventure"],
      }),
    ).toContain("horror");

    for (const title of [
      "The Witcher 3: Wild Hunt",
      "Grand Theft Auto V",
      "Red Dead Redemption 2",
      "Crimson Desert",
    ]) {
      expect(
        getLibraryGenreIds({ ...source, title, genres: ["Action"] }),
      ).toContain("open-world");
    }

    expect(
      matchesLibraryGenre(
        { ...source, title: "Sandbox World", genres: ["Sandbox"] },
        "open-world",
      ),
    ).toBe(true);
  });

  it("combines status and genre filters", () => {
    const games = createMockCatalog();
    const fighting = games[0];
    const other = games[1];
    const third = games[2];
    if (!fighting || !other || !third) throw new Error("expected mock games");

    const filtered = filterAndSortGames(
      [
        { ...fighting, genres: ["Fighting"], status: "playing" },
        { ...other, genres: ["Fighting"], status: "not-started" },
        { ...third, genres: ["Adventure"], status: "playing" },
      ],
      "",
      "playing",
      "title",
      "fighting",
    );

    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.id).toBe(fighting.id);
  });
});
