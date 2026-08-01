import { describe, expect, it } from "vitest";

import {
  NavigationLevelCoordinator,
  type NavigationRegionEntry,
} from "./navigation-hierarchy";

function entry(
  focusId: string,
  regionId: string,
  options: Partial<NavigationRegionEntry> = {},
): NavigationRegionEntry {
  return { focusId, regionId, ...options };
}

describe("NavigationLevelCoordinator", () => {
  it("restores a child's last focus before its configured entry", () => {
    const coordinator = new NavigationLevelCoordinator();
    const homeTab = entry("main-nav-home", "main-navigation", {
      childRegionId: "home-content",
      entryFocusId: "home-continue-first",
    });
    const contentEntries = [
      entry("home-continue-first", "home-content"),
      entry("home-continue-third", "home-content"),
    ];

    coordinator.recordFocus(contentEntries[1]!, contentEntries[1]!.focusId);
    expect(coordinator.resolveChild(homeTab, contentEntries)).toBe(
      "home-continue-third",
    );
  });

  it("resolves the parent through an explicit exit focus", () => {
    const coordinator = new NavigationLevelCoordinator();
    const entries = [
      entry("main-nav-home", "main-navigation"),
      entry("home-continue-first", "home-content", {
        parentRegionId: "main-navigation",
        exitFocusId: "main-nav-home",
      }),
    ];

    expect(coordinator.resolveParent(entries[1]!, entries)).toBe(
      "main-nav-home",
    );
  });

  it("remembers preferred positions without storing DOM nodes", () => {
    const coordinator = new NavigationLevelCoordinator();
    coordinator.recordFocus(
      { regionId: "home-content", parentRegionId: "main-navigation" },
      "home-continue-fourth",
      3,
    );

    expect(coordinator.getPreferredItemIndex("home-content")).toBe(3);
    expect(coordinator.getSnapshot().lastFocusedByRegion).toEqual({
      "home-content": "home-continue-fourth",
    });
  });
});
