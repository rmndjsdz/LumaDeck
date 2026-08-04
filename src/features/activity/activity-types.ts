export type ActivityStatus = "ready" | "no-data" | "offline" | "unavailable";

export interface ActivitySession {
  id: number;
  startedAt: string;
  endedAt: string | null;
  durationSeconds: number | null;
  status: "active" | "completed" | "interrupted" | string;
  source: string;
}

export interface ActivityEvent {
  id: string;
  eventType: string;
  occurredAt: string;
  title: string;
  description: string | null;
  value: unknown;
  source: string;
}

export interface ActivityStat {
  key: string;
  label: string;
  value: unknown;
  source: string;
}

export interface ActivityStreak {
  currentDays: number;
  bestDays: number;
}

export interface ActivityFriend {
  steamId64: string;
  personaName: string;
  avatarUrl: string;
  personaState: string;
  gameName: string | null;
  gameId: string | null;
}

export interface ActivitySourceStatus {
  source: string;
  status: string;
  error: string | null;
}

export interface ActivitySnapshot {
  status: ActivityStatus | string;
  metrics: {
    totalPlaytimeMinutes: number;
    lastPlayedAt: string | null;
    progress: number;
    achievementTotal: number | null;
    achievementUnlocked: number | null;
    activePlayers: number | null;
  } | null;
  lastSession: ActivitySession | null;
  sessions: ActivitySession[];
  events: ActivityEvent[];
  stats: ActivityStat[];
  streak: ActivityStreak;
  friends: ActivityFriend[];
  friendsStatus: ActivityStatus | string;
  sources: ActivitySourceStatus[];
}
