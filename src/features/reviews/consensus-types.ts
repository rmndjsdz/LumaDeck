export type ConsensusAgreement =
  "high" | "moderate" | "divided" | "polarized" | "insufficient_data";

export interface GameReviewConsensusSources {
  metacriticIncluded: boolean;
  opencriticIncluded: boolean;
  steamIncluded: boolean;
  criticReviewCount: number | null;
  playerReviewCount: number | null;
  sampledSteamReviews: number;
}

export interface GameReviewConsensus {
  gameId: string;
  overallRating: number | null;
  agreement: ConsensusAgreement;
  agreementLabel: string;
  strengths: string[];
  weaknesses: string[];
  conclusion: string;
  sources: GameReviewConsensusSources;
  generatedAt: string;
  promptVersion: number;
  providerId: string;
  modelId: string | null;
  inputFingerprint: string;
}

export interface ReviewConsensusQueryData {
  consensus: GameReviewConsensus | null;
  aiConfigured: boolean;
}
