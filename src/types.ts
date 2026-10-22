export type GameFilter = "all" | "favorites" | "active";
export type ThemeMode = "system" | "dark" | "light";
export type LanguageMode = "system" | "en" | "es";

export interface Game {
  id: string;
  name: string;
  appId: string;
  exe: string;
  favorite: boolean;
  running: boolean;
  remaining?: string;
  initials: string;
  art: [string, string];
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

export function makeGameKey(game: Pick<Game, "appId" | "exe">): string {
  return `${game.appId}::${game.exe}`;
}
