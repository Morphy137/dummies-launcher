export type GameFilter = "all" | "favorites" | "active";
export type ThemeMode = "system" | "dark" | "light";
export type LanguageMode = "system" | "en" | "es";

export interface LibraryGame {
  appId: string;
  name: string;
  exe: string;
  isFavorite: boolean;
  steamAppId?: string | null;
  iconHash?: string | null;
  coverImageHash?: string | null;
  requiresSteam: boolean;
  useSteamPath: boolean;
  installDir?: string | null;
  steamExePath?: string | null;
  steamLibraryPath?: string | null;
}

export interface Game extends LibraryGame {
  running: boolean;
  remaining?: string;
  phase?: "playing" | "syncing" | "complete";
  percent?: number;
  paused?: boolean;
}

export interface Settings {
  theme: ThemeMode;
  language: LanguageMode;
  showDummyWindow: boolean;
  minimizeToTray: boolean;
  cardsPerRow: number;
  maxCardWidth: number;
  questTimerEnabled: boolean;
  questDurationMinutes: number;
  questSyncBufferSeconds: number;
  autoStopOnComplete: boolean;
  notifyOnComplete: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  language: "system",
  showDummyWindow: true,
  minimizeToTray: true,
  cardsPerRow: 6,
  maxCardWidth: 180,
  questTimerEnabled: true,
  questDurationMinutes: 15,
  questSyncBufferSeconds: 60,
  autoStopOnComplete: true,
  notifyOnComplete: true,
};

export interface Bootstrap {
  settings: Settings;
  systemLocale: string;
  runningKeys: string[];
  library: LibraryGame[];
  version: string;
  portable: boolean;
  questTicks: QuestTick[];
}

export interface LaunchRequest {
  appId: string;
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
  code: number | null;
}

export interface QuestTick {
  key: string;
  name: string;
  timerId: number;
  remainingSeconds: number;
  phase: "playing" | "syncing" | "complete";
  percent: number;
  paused: boolean;
}

export interface SteamSetupRequest {
  appId: string;
  currentExe: string;
  libraryPath: string | null;
  steamAppId: string;
  installDir: string;
  exe: string;
}

export interface SteamSetupResult {
  manifestPath: string;
  destExePath: string;
  steamPath: string;
  updatedGame: LibraryGame;
}

export interface SteamRemoveResult {
  removedManifest: boolean;
  removedFolder: boolean;
  updatedGame?: LibraryGame | null;
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
