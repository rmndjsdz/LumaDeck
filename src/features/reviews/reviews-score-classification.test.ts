import { describe, expect, it } from "vitest";
import {
  getCriticScoreClassification,
  getSteamScoreClassification,
} from "./reviews-score-classification";

describe("critic score classifications", () => {
  it("uses the official Metacritic ranges", () => {
    expect(getCriticScoreClassification("metacritic", 92)?.label).toBe(
      "Aclamación universal",
    );
    expect(getCriticScoreClassification("metacritic", 84)?.label).toBe(
      "Generalmente favorable",
    );
    expect(getCriticScoreClassification("metacritic", 60)?.label).toBe(
      "Reseñas mixtas o promedio",
    );
    expect(getCriticScoreClassification("metacritic", 35)?.label).toBe(
      "Generalmente desfavorable",
    );
    expect(getCriticScoreClassification("metacritic", 10)?.label).toBe(
      "Desagrado generalizado",
    );
  });

  it("uses the OpenCritic ranges", () => {
    expect(getCriticScoreClassification("opencritic", 88)?.label).toBe(
      "Altamente recomendado",
    );
    expect(getCriticScoreClassification("opencritic", 80)?.label).toBe(
      "Recomendado",
    );
    expect(getCriticScoreClassification("opencritic", 65)?.label).toBe(
      "Recepción mixta",
    );
    expect(getCriticScoreClassification("opencritic", 45)?.label).toBe(
      "Recepción débil",
    );
    expect(getCriticScoreClassification("metacritic", null)).toBeNull();
  });

  it("uses Steam's native positive-review ranges", () => {
    expect(getSteamScoreClassification(96)?.label).toBe(
      "Extremadamente positivas",
    );
    expect(getSteamScoreClassification(82)?.label).toBe("Muy positivas");
    expect(getSteamScoreClassification(70)?.label).toBe("Mayormente positivas");
    expect(getSteamScoreClassification(50)?.label).toBe("Variadas");
    expect(getSteamScoreClassification(25)?.label).toBe("Mayormente negativas");
    expect(getSteamScoreClassification(10)?.label).toBe("Muy negativas");
    expect(getSteamScoreClassification(2)?.label).toBe(
      "Extremadamente negativas",
    );
    expect(getSteamScoreClassification(null)).toBeNull();
  });
});
