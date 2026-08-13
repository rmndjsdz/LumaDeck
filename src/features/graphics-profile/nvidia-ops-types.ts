export type NvidiaOpsStatus =
  | "AVAILABLE"
  | "BELOW_MIN_SPEC"
  | "UNSUPPORTED"
  | "CACHE_MISSING"
  | "NVIDIA_APP_NOT_FOUND"
  | "GAME_NOT_FOUND"
  | "AMBIGUOUS"
  | "PARSE_ERROR";

export type NvidiaOpsSetting = {
  canonicalKey: string;
  displayName: string;
  value: string;
  rawKey: string;
  rawValue: string;
};

export type NvidiaOpsProfile = {
  source: "NVIDIA_OPTIMAL_PLAYABLE_SETTINGS";
  sourceVersion: string | null;
  sourceFingerprint: string;
  resolution: { width: number; height: number } | null;
  popIndex: number;
  belowMinSpec: boolean;
  settings: NvidiaOpsSetting[];
  confidence: "HIGH" | "MEDIUM" | "LOW";
};

export type NvidiaOpsResponse = {
  status: NvidiaOpsStatus;
  game: {
    steamAppId: number | null;
    shortName: string;
    cmsId: number | null;
    executable: string | null;
    isOpsSupported: boolean;
  } | null;
  profile: NvidiaOpsProfile | null;
  diagnostic: string | null;
};
