import { describe, expect, it } from "vitest";
import {
  DETAILS_ATOMIC_CONTENT,
  DETAILS_TAB_ORDER,
} from "./details-view-contract";

describe("Details Atomic View contract", () => {
  it("keeps the product tab order stable", () => {
    expect(DETAILS_TAB_ORDER).toEqual([
      "summary",
      "performance",
      "activity",
      "achievements",
      "news",
      "dlc",
      "related",
      "reviews",
    ]);
  });

  it("keeps technical content out of Summary", () => {
    expect(DETAILS_ATOMIC_CONTENT.summary).toEqual([
      "description",
      "features",
      "screenshots",
    ]);
    expect(DETAILS_ATOMIC_CONTENT.performance).toEqual([
      "capabilities",
      "recommended-profile",
    ]);
  });
});
