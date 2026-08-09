export type GameSessionState =
  | "idle"
  | "preparing"
  | "launching"
  | "running"
  | "finishing"
  | "error"
  | "unsupported";

export type MonitoringMode = "full" | "compatible";

export interface SessionCapabilities {
  playtime: boolean;
  startTime: boolean;
  endTime: boolean;
  processTracking: boolean;
  advancedProcessMetrics: boolean;
}

export interface GameSessionStatus {
  sessionId: string;
  gameId: string;
  steamAppId: number;
  source: string;
  state: GameSessionState;
  occurredAt: string;
  elapsedSeconds: number;
  message: string;
  unsupportedReason?: string | null;
  monitoringMode: MonitoringMode;
  antiCheatProvider?: string | null;
  compatibleReason?: string | null;
  capabilities: SessionCapabilities;
}
