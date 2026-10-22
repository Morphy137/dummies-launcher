export type GameFilter = "all" | "favorites" | "active";
export type ThemeMode = "system" | "dark" | "light";
export type LanguageMode = "system" | "en" | "es";

export interface Game {
  appId: string;
  name: string;
  exe: string;
  isFavorite: boolean;
  running: boolean;
  remaining?: string;
  steamAppId?: string | null;
  iconHash?: string | null;
  coverImageHash?: string | null;
  requiresSteam: boolean;
  useSteamPath: boolean;
  installDir?: string | null;
  steamExePath?: string | null;
}

export interface Settings {
  theme: ThemeMode;
  language: LanguageMode;
  showDummyWindow: boolean;
  minimizeToTray: boolean;
  cardsPerRow: number;
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  language: "system",
  showDummyWindow: false,
  minimizeToTray: true,
  cardsPerRow: 6,
};

export interface Bootstrap {
  settings: Settings;
  systemLocale: string;
  runningKeys: string[];
  library: Game[];
  version: string;
}

export interface LaunchRequest {
  appId: string;
  name: string;
  exe: string;
}

export interface LaunchResult {
  gameKey: string;
  exePath: string;
  alreadyRunning: boolean;
}

export interface DummyExitedPayload {
  key: string;
  name: string;
  code?: number;
}

export interface DatabaseGame {
  id: string;
  name: string;
  exe: string;
  isLauncher: boolean;
  usesNewDetection: boolean;
  steamAppId?: string | null;
  iconHash?: string | null;
  coverImageHash?: string | null;
}

export interface DatabasePage {
  items: DatabaseGame[];
  offset: number;
  limit: number;
  hasMore: boolean;
}

export interface SyncResult {
  updated: boolean;
  count: number;
  path: string;
}

export function makeGameKey(game: Pick<Game, "appId" | "exe">): string {
  return `${game.appId}::${game.exe}`;
}
