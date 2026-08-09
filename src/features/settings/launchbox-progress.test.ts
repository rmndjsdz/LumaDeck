import { describe, expect, it } from "vitest";
import {
  formatLaunchBoxDuration,
  launchBoxPhaseCopy,
  launchBoxProgressPercent,
} from "./launchbox-progress";
import type { LaunchBoxCatalogProgress } from "./provider-settings-service";

function progress(
  overrides: Partial<LaunchBoxCatalogProgress> = {},
): LaunchBoxCatalogProgress {
  return {
    phase: "importing",
    processedRecords: 61_000,
    totalRecords: null,
    downloadedBytes: null,
    totalBytes: null,
    elapsedMs: 65_000,
    lastProgressAtMs: Date.now(),
    ...overrides,
  };
}

describe("LaunchBox catalog progress", () => {
  it("maps every backend phase to user-facing copy", () => {
    expect(launchBoxPhaseCopy(progress({ phase: "downloading" })).title).toBe(
      "Descargando catálogo",
    );
    expect(launchBoxPhaseCopy(progress({ phase: "extracting" })).title).toBe(
      "Preparando catálogo",
    );
    expect(launchBoxPhaseCopy(progress({ phase: "importing" })).title).toBe(
      "Importando metadatos",
    );
    expect(launchBoxPhaseCopy(progress({ phase: "validating" })).title).toBe(
      "Validando catálogo",
    );
    expect(launchBoxPhaseCopy(progress({ phase: "activating" })).title).toBe(
      "Activando catálogo",
    );
  });

  it("uses real bytes or records and stays indeterminate without totals", () => {
    expect(
      launchBoxProgressPercent(
        progress({
          phase: "downloading",
          downloadedBytes: 50,
          totalBytes: 100,
        }),
      ),
    ).toBe(50);
    expect(launchBoxProgressPercent(progress())).toBeNull();
    expect(
      launchBoxProgressPercent(
        progress({ processedRecords: 61_000, totalRecords: 99_842 }),
      ),
    ).toBe(61);
  });

  it("formats elapsed time without estimating completion", () => {
    expect(formatLaunchBoxDuration(65_000)).toBe("1m 05s");
  });
});
