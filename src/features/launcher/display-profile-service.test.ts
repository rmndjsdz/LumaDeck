import { describe, expect, it } from "vitest";
import {
  formatDisplayRefreshRate,
  formatDisplayResolution,
} from "./display-profile-service";

describe("display profile presentation", () => {
  it("uses Auto when a custom mode is not selected", () => {
    expect(formatDisplayResolution(null, null)).toBe("Auto");
    expect(formatDisplayRefreshRate(null)).toBe("Auto");
  });

  it("formats the selected resolution and refresh rate", () => {
    expect(formatDisplayResolution(2560, 1440)).toBe("2560 × 1440");
    expect(formatDisplayRefreshRate(60)).toBe("60 Hz");
  });
});
