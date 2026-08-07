import type { Game } from "../catalog/game-types";

export type RecommendationSignal =
  | "franchise"
  | "studio"
  | "publisher"
  | "genres"
  | "gameplay"
  | "mechanics"
  | "setting"
  | "duration"
  | "quality"
  | "popularity"
  | "profile";

export type RecommendationWeights = Record<RecommendationSignal, number>;

export type RecommendationReason = {
  signal: RecommendationSignal;
  label: string;
  score: number;
};

export type RankedRecommendation = {
  game: Game;
  score: number;
  reasons: RecommendationReason[];
};

export const DEFAULT_RECOMMENDATION_WEIGHTS: RecommendationWeights = {
  franchise: 0.13,
  studio: 0.1,
  publisher: 0.06,
  genres: 0.16,
  gameplay: 0.14,
  mechanics: 0.1,
  setting: 0.07,
  duration: 0.08,
  quality: 0.07,
  popularity: 0.05,
  profile: 0.04,
};

const SIGNAL_LABELS: Record<RecommendationSignal, string> = {
  franchise: "Universo compartido",
  studio: "Mismo estudio",
  publisher: "Publisher relacionado",
  genres: "Géneros que disfrutas",
  gameplay: "Gameplay afín",
  mechanics: "Mecánicas compatibles",
  setting: "Ambientación compatible",
  duration: "Duración similar",
  quality: "Muy bien valorado",
  popularity: "Favorito de la comunidad",
  profile: "Encaja con tu perfil",
};

const STOP_WORDS = new Set([
  "a",
  "an",
  "and",
  "edition",
  "the",
  "of",
  "para",
  "the",
  "game",
  "remastered",
  "complete",
]);

export function rankRecommendations(
  source: Game,
  candidates: readonly Game[],
  weights: RecommendationWeights = DEFAULT_RECOMMENDATION_WEIGHTS,
): RankedRecommendation[] {
  return candidates
    .filter((candidate) => candidate.id !== source.id)
    .map((candidate) => {
      const signals = calculateSignals(source, candidate);
      const reasons = Object.entries(weights)
        .map(([signal, weight]) => {
          const typedSignal = signal as RecommendationSignal;
          return {
            signal: typedSignal,
            label: SIGNAL_LABELS[typedSignal],
            score: signals[typedSignal] * weight,
          } satisfies RecommendationReason;
        })
        .filter((reason) => reason.score >= 0.018)
        .sort((left, right) => right.score - left.score)
        .slice(0, 5);
      const score = Math.round(
        clamp(
          Object.entries(weights).reduce(
            (total, [signal, weight]) =>
              total + signals[signal as RecommendationSignal] * weight,
            0,
          ) * 100,
          1,
          99,
        ),
      );
      return { game: candidate, score, reasons } satisfies RankedRecommendation;
    })
    .sort((left, right) => {
      if (right.score !== left.score) return right.score - left.score;
      return left.game.title.localeCompare(right.game.title);
    });
}

export function getRecommendationReasons(
  recommendation: RankedRecommendation,
): string[] {
  return recommendation.reasons.map((reason) => reason.label);
}

function calculateSignals(
  source: Game,
  candidate: Game,
): Record<RecommendationSignal, number> {
  const sourceSteam = source.details?.steam;
  const candidateSteam = candidate.details?.steam;
  const sourceGenres = collect(source.genres, sourceSteam?.genres);
  const candidateGenres = collect(candidate.genres, candidateSteam?.genres);
  const sourceTags = collect(sourceSteam?.tags, sourceSteam?.categories);
  const candidateTags = collect(
    candidateSteam?.tags,
    candidateSteam?.categories,
  );
  const sourceDevelopers = sourceSteam?.developers ?? [];
  const candidateDevelopers = candidateSteam?.developers ?? [];
  const sourcePublishers = sourceSteam?.publishers ?? [];
  const candidatePublishers = candidateSteam?.publishers ?? [];
  const sourceTokens = titleTokens(source.title);
  const candidateTokens = titleTokens(candidate.title);
  const sourceDuration = durationMinutes(source);
  const candidateDuration = durationMinutes(candidate);

  return {
    franchise: overlap(sourceTokens, candidateTokens, true),
    studio: overlap(sourceDevelopers, candidateDevelopers),
    publisher: overlap(sourcePublishers, candidatePublishers),
    genres: overlap(sourceGenres, candidateGenres),
    gameplay: overlap(sourceTags, candidateTags),
    mechanics: overlap(
      [...sourceTags, ...sourceGenres],
      [...candidateTags, ...candidateGenres],
    ),
    setting: settingMatch(sourceTags, candidateTags),
    duration: durationSimilarity(sourceDuration, candidateDuration),
    quality: qualityScore(candidate),
    popularity: popularityScore(candidate),
    profile: profileCompatibility(source, candidate),
  };
}

function collect(...groups: (readonly string[] | undefined)[]): string[] {
  return [...new Set(groups.flatMap((group) => group ?? []))];
}

function normalize(value: string): string {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLocaleLowerCase()
    .replace(/[^a-z0-9 ]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function titleTokens(title: string): string[] {
  return normalize(title)
    .split(" ")
    .filter((token) => token.length > 2 && !STOP_WORDS.has(token));
}

function overlap(
  leftValues: readonly string[],
  rightValues: readonly string[],
  requireMeaningfulToken = false,
): number {
  const left = new Set(leftValues.map(normalize).filter(Boolean));
  const right = new Set(rightValues.map(normalize).filter(Boolean));
  if (left.size === 0 || right.size === 0) return 0;
  const shared = [...left].filter((value) => {
    if (requireMeaningfulToken && value.length < 4) return false;
    return right.has(value);
  }).length;
  return clamp(shared / Math.max(left.size, right.size), 0, 1);
}

function settingMatch(
  sourceTags: readonly string[],
  candidateTags: readonly string[],
): number {
  const settingTerms = new Set([
    "japan",
    "japanese",
    "medieval",
    "fantasy",
    "sci fi",
    "science fiction",
    "cyberpunk",
    "space",
    "western",
    "horror",
    "historical",
    "post apocalyptic",
  ]);
  const sourceSettings = sourceTags
    .map(normalize)
    .filter((tag) => settingTerms.has(tag));
  const candidateSettings = candidateTags
    .map(normalize)
    .filter((tag) => settingTerms.has(tag));
  return overlap(sourceSettings, candidateSettings);
}

function durationMinutes(game: Game): number {
  return (
    game.details?.hltb?.mainStoryMinutes ?? Math.max(game.playtimeMinutes, 1)
  );
}

function durationSimilarity(source: number, candidate: number): number {
  const difference = Math.abs(source - candidate);
  return clamp(1 - difference / Math.max(source, candidate, 1), 0, 1);
}

function qualityScore(game: Game): number {
  const reviewScore = game.details?.steam?.reviewScore;
  if (reviewScore !== null && reviewScore !== undefined) {
    return clamp(reviewScore / 100, 0, 1);
  }
  return game.status === "completed" ? 0.86 : game.favorite ? 0.8 : 0.62;
}

function popularityScore(game: Game): number {
  const reviewCount = game.details?.steam?.reviewCount;
  if (reviewCount !== null && reviewCount !== undefined) {
    return clamp(Math.log10(reviewCount + 1) / 6, 0, 1);
  }
  return game.favorite ? 0.9 : game.installed ? 0.7 : 0.45;
}

function profileCompatibility(source: Game, candidate: Game): number {
  const sourceSignals = [
    source.favorite,
    source.installed,
    source.status === "completed",
  ];
  const candidateSignals = [
    candidate.favorite,
    candidate.installed,
    candidate.status === "completed",
  ];
  return (
    sourceSignals.reduce(
      (score, value, index) =>
        score + (value === candidateSignals[index] ? 1 : 0),
      0,
    ) / sourceSignals.length
  );
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}
