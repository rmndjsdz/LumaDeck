import { describe, expect, it } from "vitest";
import { formatHltbDuration } from "./hltb-format";

describe("formatHltbDuration", () => {
  it.each([
    [45, "1 h"],
    [120, "2 h"],
    [150, "3 h"],
    [840, "14 h"],
    [7200, "120 h"],
  ])("formats %i minutes", (minutes, expected) => {
    expect(formatHltbDuration(minutes)).toBe(expected);
  });

  it("does not turn missing data into zero hours", () => {
    expect(formatHltbDuration(null)).toBe("Sin datos");
    expect(formatHltbDuration(0)).toBe("Sin datos");
  });
});
