import { describe, expect, it } from "vitest";
import { activityErrorMessage } from "./activity-service";

describe("activityErrorMessage", () => {
  it("keeps local activity useful when Steam is offline", () => {
    expect(activityErrorMessage(new Error("STEAM_OFFLINE"))).toContain(
      "datos locales",
    );
  });

  it("distinguishes an unconfigured desktop runtime", () => {
    expect(
      activityErrorMessage(new Error("ACTIVITY_RUNTIME_UNAVAILABLE")),
    ).toContain("aplicación de escritorio");
  });

  it("does not expose raw provider errors to the view", () => {
    expect(activityErrorMessage(new Error("unexpected-provider-detail"))).toBe(
      "No se pudo consultar la actividad.",
    );
  });
});
