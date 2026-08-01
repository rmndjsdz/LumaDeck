export type GameStatus = "not-started" | "playing" | "completed";

export type Game = {
  id: string;
  title: string;
  sortTitle: string;
  platform: string;
  provider: string;
  coverUrl: string;
  backgroundUrl: string;
  description: string;
  genres: string[];
  releaseYear: number;
  playtimeMinutes: number;
  lastPlayedAt: string | null;
  favorite: boolean;
  installed: boolean;
  progress: number;
  status: GameStatus;
};
