import { describe, expect, it } from "vitest";
import { reviewsQueryKey } from "./reviews-query";

describe("reviews query key", () => {
  it("is stable for the same game identity and ignores mutable title data", () => {
    expect(reviewsQueryKey("steam-440")).toEqual([
      "reviews-summary",
      "steam-440",
    ]);
    expect(reviewsQueryKey("steam-440")).toEqual(reviewsQueryKey("steam-440"));
    expect(reviewsQueryKey("steam-440")).not.toEqual(
      reviewsQueryKey("steam-730"),
    );
  });
});
