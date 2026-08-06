import type { ReviewProvider } from "./reviews-types";

export type CriticScoreClassification = {
  label: string;
  tone:
    | "acclaim"
    | "favorable"
    | "mixed"
    | "unfavorable"
    | "dislike"
    | "highly-recommended"
    | "recommended"
    | "weak";
  color: string;
};

export type SteamScoreClassification = {
  label: string;
  tone:
    | "extremely-positive"
    | "very-positive"
    | "mostly-positive"
    | "varied"
    | "mostly-negative"
    | "very-negative"
    | "extremely-negative";
  color: string;
};

export function getCriticScoreClassification(
  provider: ReviewProvider,
  score: number | null,
): CriticScoreClassification | null {
  if (score === null || provider === "steam") return null;

  if (provider === "metacritic") {
    if (score >= 90) {
      return {
        label: "Aclamación universal",
        tone: "acclaim",
        color: "#22C55E",
      };
    }
    if (score >= 75) {
      return {
        label: "Generalmente favorable",
        tone: "favorable",
        color: "#22C55E",
      };
    }
    if (score >= 50) {
      return {
        label: "Reseñas mixtas o promedio",
        tone: "mixed",
        color: "#FACC15",
      };
    }
    if (score >= 20) {
      return {
        label: "Generalmente desfavorable",
        tone: "unfavorable",
        color: "#F97316",
      };
    }
    return {
      label: "Desagrado generalizado",
      tone: "dislike",
      color: "#EF4444",
    };
  }

  if (score >= 90) {
    return {
      label: "Aclamación universal",
      tone: "acclaim",
      color: "#22C55E",
    };
  }
  if (score >= 85) {
    return {
      label: "Altamente recomendado",
      tone: "highly-recommended",
      color: "#22C55E",
    };
  }
  if (score >= 75) {
    return {
      label: "Recomendado",
      tone: "recommended",
      color: "#3B82F6",
    };
  }
  if (score >= 60) {
    return {
      label: "Recepción mixta",
      tone: "mixed",
      color: "#FACC15",
    };
  }
  return {
    label: "Recepción débil",
    tone: "weak",
    color: "#EF4444",
  };
}

export function getSteamScoreClassification(
  score: number | null,
): SteamScoreClassification | null {
  if (score === null) return null;
  if (score >= 95) {
    return {
      label: "Extremadamente positivas",
      tone: "extremely-positive",
      color: "#2ECC71",
    };
  }
  if (score >= 80) {
    return {
      label: "Muy positivas",
      tone: "very-positive",
      color: "#38BDF8",
    };
  }
  if (score >= 60) {
    return {
      label: "Mayormente positivas",
      tone: "mostly-positive",
      color: "#7DD3FC",
    };
  }
  if (score >= 40) {
    return {
      label: "Variadas",
      tone: "varied",
      color: "#FACC15",
    };
  }
  if (score >= 20) {
    return {
      label: "Mayormente negativas",
      tone: "mostly-negative",
      color: "#FB923C",
    };
  }
  if (score >= 5) {
    return {
      label: "Muy negativas",
      tone: "very-negative",
      color: "#F87171",
    };
  }
  return {
    label: "Extremadamente negativas",
    tone: "extremely-negative",
    color: "#EF4444",
  };
}
