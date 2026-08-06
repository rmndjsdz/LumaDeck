import { describe, expect, it } from "vitest";
import { categoryLabel, filterCategories, NEWS_FILTERS } from "./news-types";
import { newsErrorMessage, safeNewsUrl } from "./news-service";

describe("news filters", () => {
  it("keeps the approved four-filter rail", () => {
    expect(NEWS_FILTERS.map((filter) => filter.id)).toEqual([
      "all",
      "official",
      "updates",
      "community",
    ]);
  });

  it("maps update feed categories without exposing extra filters", () => {
    expect(filterCategories("updates")).toEqual([
      "update",
      "event",
      "dlc",
      "maintenance",
    ]);
  });

  it("keeps backend category labels presentational", () => {
    expect(categoryLabel("community")).toBe("COMUNIDAD");
    expect(categoryLabel("maintenance")).toBe("MANTENIMIENTO");
  });
});

describe("news source safety", () => {
  it("allows only secure Steam source URLs", () => {
    expect(safeNewsUrl("https://store.steampowered.com/news/1")).not.toBeNull();
    expect(
      safeNewsUrl(
        "https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/1",
      ),
    ).not.toBeNull();
    expect(safeNewsUrl("https://cdn.akamaihd.net/news/1")).toBeNull();
    expect(safeNewsUrl("http://store.steampowered.com/news/1")).toBeNull();
    expect(safeNewsUrl("https://example.com/news/1")).toBeNull();
  });

  it("does not expose provider errors", () => {
    expect(newsErrorMessage(new Error("unexpected-provider-detail"))).toBe(
      "No se pudieron cargar las noticias.",
    );
  });
});
