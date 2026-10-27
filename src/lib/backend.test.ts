import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS } from "../types";
import * as backend from "./backend";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: () => true }));

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue(undefined);
});

describe("backend IPC contract", () => {
  it("looks up artwork without changing the game's Steam integration", async () => {
    await backend.resolveSteamArtwork("ARKNIGHTS: ENDFIELD");
    expect(invoke).toHaveBeenLastCalledWith("resolve_steam_artwork", {
      name: "ARKNIGHTS: ENDFIELD",
      steamAppId: null,
    });
    await backend.resolveSteamArtwork("Aniimo", "4126040");
    expect(invoke).toHaveBeenLastCalledWith("resolve_steam_artwork", {
      name: "Aniimo",
      steamAppId: "4126040",
    });
  });
  it.each([
    ["get_bootstrap", () => backend.getBootstrap()],
    ["get_library", () => backend.getLibrary()],
    ["get_steam_path", () => backend.getSteamPath()],
    ["get_steam_libraries", () => backend.getSteamLibraries()],
    ["list_running_dummies", () => backend.listRunningDummies()],
    ["stop_all_dummies", () => backend.stopAllDummies()],
  ] as const)("keeps %s registered without an argument payload", async (command, call) => {
    await call();
    expect(invoke).toHaveBeenCalledExactlyOnceWith(command);
  });

  it.each([
    ["remove_game", backend.removeGame],
    ["toggle_favorite", backend.toggleFavorite],
    ["create_shortcut", backend.createShortcut],
    ["remove_steam_integration", backend.removeSteamIntegration],
  ] as const)("passes the full identity to %s", async (command, call) => {
    await call("123", "original.exe");
    expect(invoke).toHaveBeenCalledExactlyOnceWith(command, { appId: "123", exe: "original.exe" });
  });

  it.each([
    ["stop_dummy", backend.stopDummy],
    ["pause_dummy", backend.pauseDummy],
    ["resume_dummy", backend.resumeDummy],
  ] as const)("passes the running key to %s", async (command, call) => {
    await call("123::original.exe");
    expect(invoke).toHaveBeenCalledExactlyOnceWith(command, { gameKey: "123::original.exe" });
  });

  it("keeps nested Steam and launch requests distinct from flat identity commands", async () => {
    const request = {
      appId: "123",
      currentExe: "original.exe",
      steamAppId: "456",
      installDir: "DedicatedDummy",
      exe: "bin/game.exe",
      libraryPath: null,
    };
    await backend.setupSteamIntegration(request);
    expect(invoke).toHaveBeenLastCalledWith("setup_steam_integration", { request });
    await backend.launchDummy({ appId: "123", exe: "original.exe" });
    expect(invoke).toHaveBeenLastCalledWith("launch_dummy", {
      request: { appId: "123", exe: "original.exe" },
    });
  });

  it("retains settings, catalog paging and executable-edit argument names", async () => {
    await backend.saveSettings(DEFAULT_SETTINGS);
    expect(invoke).toHaveBeenLastCalledWith("save_settings", { settings: DEFAULT_SETTINGS });
    await backend.getDatabasePage("example", 60, 60);
    expect(invoke).toHaveBeenLastCalledWith("get_database_page", {
      filter: "example",
      offset: 60,
      limit: 60,
    });
    await backend.updateGameExecutable("123", "old.exe", "new.exe");
    expect(invoke).toHaveBeenLastCalledWith("update_game_executable", {
      appId: "123",
      oldExe: "old.exe",
      newExe: "new.exe",
    });
    await backend.addGame({
      id: "123",
      name: "Example",
      exe: "original.exe",
      isLauncher: false,
      usesNewDetection: true,
    });
    expect(invoke).toHaveBeenLastCalledWith("add_game", {
      request: {
        id: "123",
        name: "Example",
        exe: "original.exe",
        usesNewDetection: true,
        steamAppId: null,
        iconHash: null,
        coverImageHash: null,
      },
    });
  });

  it("keeps catalog synchronization serialized through the same command", async () => {
    vi.mocked(invoke).mockResolvedValue({ updated: false, count: 1, path: "cache" });
    await Promise.all([backend.syncGamelist(), backend.syncGamelist()]);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("sync_gamelist");
  });
});
