import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  type Bootstrap,
  type DatabaseGame,
  type DatabasePage,
  DEFAULT_SETTINGS,
  type DummyExitedPayload,
  type Game,
  type LaunchRequest,
  type LaunchResult,
  type QuestTick,
  type Settings,
  type SteamRemoveResult,
  type SteamSetupRequest,
  type SteamSetupResult,
  type SyncResult,
  type UpdateInfo,
} from "../types";

function browserBootstrap(): Bootstrap {
  return {
    settings: DEFAULT_SETTINGS,
    systemLocale: navigator.language || "en",
    runningKeys: [],
    library: [],
    version: "dev",
  };
}

export async function getBootstrap(): Promise<Bootstrap> {
  if (!isTauri()) return browserBootstrap();
  return invoke<Bootstrap>("get_bootstrap");
}

export async function saveSettings(settings: Settings): Promise<Settings> {
  if (!isTauri()) return settings;
  return invoke<Settings>("save_settings", { settings });
}

export async function syncGamelist(): Promise<SyncResult> {
  if (!isTauri()) {
    return { updated: false, count: 0, path: "" };
  }
  return invoke<SyncResult>("sync_gamelist");
}

export async function getDatabasePage(
  filter: string,
  offset: number,
  limit: number,
): Promise<DatabasePage> {
  if (!isTauri()) return { items: [], offset, limit, hasMore: false };
  return invoke<DatabasePage>("get_database_page", { filter, offset, limit });
}

export async function getLibrary(): Promise<Game[]> {
  if (!isTauri()) return [];
  return invoke<Game[]>("get_library");
}

export async function addGame(game: DatabaseGame): Promise<Game> {
  if (!isTauri()) throw new Error("The desktop backend is not available in the browser preview.");
  return invoke<Game>("add_game", {
    request: {
      id: game.id,
      name: game.name,
      exe: game.exe,
      usesNewDetection: game.usesNewDetection,
      steamAppId: game.steamAppId ?? null,
      iconHash: game.iconHash ?? null,
      coverImageHash: game.coverImageHash ?? null,
    },
  });
}

export async function removeGame(appId: string, exe: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("remove_game", { appId, exe });
}

export async function toggleFavorite(appId: string, exe: string): Promise<Game> {
  if (!isTauri()) throw new Error("The desktop backend is not available in the browser preview.");
  return invoke<Game>("toggle_favorite", { appId, exe });
}

export async function updateGameExecutable(
  appId: string,
  oldExe: string,
  newExe: string,
): Promise<Game> {
  if (!isTauri()) throw new Error("The desktop backend is not available in the browser preview.");
  return invoke<Game>("update_game_executable", { appId, oldExe, newExe });
}

export async function createShortcut(appId: string, exe: string): Promise<string> {
  if (!isTauri()) throw new Error("The desktop backend is not available in the browser preview.");
  return invoke<string>("create_shortcut", { appId, exe });
}

export async function getSteamPath(): Promise<string | null> {
  if (!isTauri()) return null;
  return invoke<string | null>("get_steam_path");
}

export async function setupSteamIntegration(request: SteamSetupRequest): Promise<SteamSetupResult> {
  if (!isTauri()) throw new Error("The desktop backend is not available in the browser preview.");
  return invoke<SteamSetupResult>("setup_steam_integration", { ...request });
}

export async function removeSteamIntegration(
  appId: string,
  steamAppId?: string | null,
  installDir?: string | null,
): Promise<SteamRemoveResult> {
  if (!isTauri()) throw new Error("The desktop backend is not available in the browser preview.");
  return invoke<SteamRemoveResult>("remove_steam_integration", {
    appId,
    steamAppId: steamAppId ?? null,
    installDir: installDir ?? null,
  });
}

export async function checkForUpdates(): Promise<UpdateInfo> {
  if (!isTauri()) {
    return {
      updateAvailable: false,
      currentVersion: "dev",
      latestVersion: "dev",
      releaseUrl: "",
      releaseName: "",
      releaseNotes: "",
    };
  }
  return invoke<UpdateInfo>("check_for_updates");
}

export async function launchDummy(request: LaunchRequest): Promise<LaunchResult> {
  if (!isTauri()) {
    throw new Error("The desktop backend is not available in the browser preview.");
  }
  return invoke<LaunchResult>("launch_dummy", { request });
}

export async function stopDummy(gameKey: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("stop_dummy", { gameKey });
}

export async function stopAllDummies(): Promise<number> {
  if (!isTauri()) return 0;
  return invoke<number>("stop_all_dummies");
}

export async function listRunningDummies(): Promise<string[]> {
  if (!isTauri()) return [];
  return invoke<string[]>("list_running_dummies");
}

export function onDummyExited(handler: (payload: DummyExitedPayload) => void): () => void {
  if (!isTauri()) return () => {};

  let unlisten: (() => void) | undefined;
  void listen<DummyExitedPayload>("dummy://exited", (event) => handler(event.payload)).then(
    (fn) => {
      unlisten = fn;
    },
  );

  return () => {
    unlisten?.();
  };
}

export function onQuestTick(handler: (payload: QuestTick) => void): () => void {
  if (!isTauri()) return () => {};

  let unlisten: (() => void) | undefined;
  void listen<QuestTick>("quest://tick", (event) => handler(event.payload)).then((fn) => {
    unlisten = fn;
  });

  return () => {
    unlisten?.();
  };
}
