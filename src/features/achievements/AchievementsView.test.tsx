import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { describe, expect, it } from "vitest";
import { AchievementsHeader, DistributionPanel } from "./AchievementsView";
import type { Achievement } from "./achievement-types";

describe("achievements progress header", () => {
  it("renders unlocked virtual trophy distribution instead of total distribution", async () => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root: Root = createRoot(host);

    await act(async () => {
      root.render(
        <AchievementsHeader
          summary={{
            total: 46,
            unlocked: 4,
            locked: 42,
            completionPercentage: 8.7,
          }}
          distributions={{
            total: { bronze: 46, silver: 2, gold: 1 },
            unlocked: { bronze: 3, silver: 1, gold: 0 },
          }}
        />,
      );
    });

    expect(
      Array.from(
        host.querySelectorAll(".achievements-header-trophy-count strong"),
      ).map((element) => element.textContent),
    ).toEqual(["0", "0", "1", "3"]);
    expect(
      host.querySelector<HTMLImageElement>(".achievements-header-trophy")?.src,
    ).toContain("/themes/cinematic/assets/plata.png");

    await act(async () => {
      root.unmount();
    });
    host.remove();
  });

  it("renders four rarity bands and merges very common into common", async () => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root: Root = createRoot(host);
    const rarities: Achievement["rarity"][] = [
      "very-rare",
      "rare",
      "uncommon",
      "common",
      "very-common",
    ];

    await act(async () => {
      root.render(
        <DistributionPanel
          achievements={rarities.map((rarity, index) =>
            createAchievement(`achievement-${index}`, rarity),
          )}
        />,
      );
    });

    expect(host.querySelectorAll(".achievement-distribution-row")).toHaveLength(
      4,
    );
    expect(
      Array.from(
        host.querySelectorAll(".achievement-distribution-row strong"),
      ).map((element) => element.textContent),
    ).toEqual(["1", "20%", "1", "20%", "1", "20%", "2", "40%"]);

    await act(async () => {
      root.unmount();
    });
    host.remove();
  });
});

function createAchievement(
  apiName: string,
  rarity: Achievement["rarity"],
): Achievement {
  return {
    apiName,
    displayName: apiName,
    description: "",
    hidden: false,
    unlocked: false,
    unlockTime: null,
    unlockPercentage: null,
    rarity,
    virtualTier: "bronze",
    iconUnlocked: null,
    iconLocked: null,
    localIconUnlocked: null,
    localIconLocked: null,
  };
}
