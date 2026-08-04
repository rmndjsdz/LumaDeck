export type GameSessionState =
  | "idle"
  | "preparing"
  | "launching"
  | "running"
  | "finishing"
  | "error"
  | "unsupported";

export interface GameSessionStatus {
  sessionId: string;
  gameId: string;
  steamAppId: number;
  state: GameSessionState;
  occurredAt: string;
  elapsedSeconds: number;
  message: string;
  unsupportedReason?: string | null;
}
