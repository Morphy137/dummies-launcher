// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import App from "../App";
import {
  getBootstrap,
  getDatabasePage,
  getSteamLibraries,
  openSteamIntegrationFolder,
  setupSteamIntegration,
  syncGamelist,
} from "../lib/backend";
import type { DatabaseGame, Game, SyncResult } from "../types";
import { DEFAULT_SETTINGS } from "../types";
import AddGameDialog from "./AddGameDialog";
import GameList from "./GameList";
import SettingsDialog from "./SettingsDialog";
import SteamDialog from "./SteamDialog";

vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) =>
      options?.count !== undefined ? `${key} ${options.count}` : key,
  }),
}));
vi.mock("../lib/backend", () => ({
  resolveSteamArtwork: vi.fn().mockResolvedValue([]),
  resolveSteamCover: vi.fn().mockResolvedValue([]),
  getDatabasePage: vi.fn(),
  syncGamelist: vi.fn(),
  getSteamLibraries: vi.fn(),
  inspectSteamIntegration: vi.fn().mockResolvedValue({
    manifestPath: "C:\\Steam\\steamapps\\appmanifest_456.acf",
    dummyPath: "C:\\Steam\\steamapps\\common\\Dummy\\old.exe",
  }),
  openSteamIntegrationFolder: vi.fn().mockResolvedValue(undefined),
  getBootstrap: vi.fn(),
  setupSteamIntegration: vi.fn(),
}));

const game: Game = {
  appId: "123",
  name: "Example",
  exe: "game.exe",
  isFavorite: false,
  running: false,
  requiresSteam: false,
  useSteamPath: true,
  steamAppId: "456",
  installDir: "Dummy",
};
const row: DatabaseGame = {
  id: "123",
  name: "Cached game",
  exe: "game.exe",
  isLauncher: false,
  usesNewDetection: false,
};

beforeAll(() => {
  window.matchMedia = vi.fn().mockImplementation(() => ({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
  }));
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("critical UI flows", () => {
  it("previews and saves the expanded row limit and maximum card width", async () => {
    const preview = vi.fn();
    const save = vi.fn().mockResolvedValue(undefined);
    render(
      <SettingsDialog
        open
        settings={DEFAULT_SETTINGS}
        version="test"
        portable={false}
        onPreview={preview}
        onCancel={vi.fn()}
        onDismiss={vi.fn()}
        onSave={save}
        onCheckForUpdates={vi.fn().mockResolvedValue("current")}
      />,
    );
    fireEvent.click(screen.getByRole("combobox", { name: "settings.cardsPerRow" }));
    fireEvent.click(await screen.findByRole("option", { name: "settings.cardsPerRowValue 24" }));
    fireEvent.click(screen.getByRole("combobox", { name: "settings.maxCardWidth" }));
    fireEvent.click(await screen.findByRole("option", { name: "settings.pixelsValue 240" }));
    expect(preview).toHaveBeenLastCalledWith(
      expect.objectContaining({ cardsPerRow: 24, maxCardWidth: 240 }),
    );
    fireEvent.click(screen.getByRole("button", { name: "common.save" }));
    await waitFor(() =>
      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({ cardsPerRow: 24, maxCardWidth: 240 }),
      ),
    );
  });
  it("retains imported timer options and treats Escape as cancellation", async () => {
    const dismiss = vi.fn();
    render(
      <SettingsDialog
        open
        settings={{ ...DEFAULT_SETTINGS, questDurationMinutes: 25, questSyncBufferSeconds: 120 }}
        version="test"
        portable={false}
        onPreview={vi.fn()}
        onCancel={vi.fn()}
        onDismiss={dismiss}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onCheckForUpdates={vi.fn().mockResolvedValue("current")}
      />,
    );
    expect(screen.getByText("settings.minutesValue 25")).toBeDefined();
    expect(screen.getByText("settings.secondsValue 120")).toBeDefined();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape", code: "Escape" });
    await waitFor(() => expect(dismiss).toHaveBeenCalledOnce());
  });

  it("replaces the old library identity after Steam changes the executable", async () => {
    const original = { ...game, useSteamPath: false };
    vi.mocked(getBootstrap).mockResolvedValue({
      settings: DEFAULT_SETTINGS,
      systemLocale: "en",
      runningKeys: [],
      library: [original],
      version: "test",
      portable: false,
      questTicks: [],
    });
    vi.mocked(getSteamLibraries).mockResolvedValue(["C:\\Steam"]);
    vi.mocked(setupSteamIntegration).mockResolvedValue({
      updatedGame: { ...original, exe: "bin\\new.exe", useSteamPath: true },
      manifestPath: "test",
      destExePath: "test",
      steamPath: "test",
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Example" }));
    fireEvent.click(screen.getByRole("button", { name: "steam.title" }));
    const executable = await screen.findByLabelText("steam.exe");
    fireEvent.change(executable, { target: { value: "bin/new.exe" } });
    const install = screen.getByRole("button", { name: "steam.install" });
    await waitFor(() => expect(install.hasAttribute("disabled")).toBe(false));
    fireEvent.click(install);
    await waitFor(() =>
      expect(setupSteamIntegration).toHaveBeenCalledWith(
        expect.objectContaining({ currentExe: "game.exe", exe: "bin/new.exe" }),
      ),
    );
    await screen.findByText("detail.steamConfigured");
    expect(screen.getAllByText("bin\\new.exe").length).toBeGreaterThan(0);
  });

  it("shows cached results without waiting for a pending catalog refresh", async () => {
    vi.mocked(syncGamelist).mockReturnValue(new Promise<SyncResult>(() => {}));
    vi.mocked(getDatabasePage).mockResolvedValue({
      items: [row],
      offset: 0,
      limit: 60,
      hasMore: false,
    });
    render(<AddGameDialog open onOpenChange={vi.fn()} onAdd={vi.fn()} />);
    await screen.findByText("Cached game");
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Cached" } });
    await waitFor(() => expect(getDatabasePage).toHaveBeenCalledWith("Cached", 0, 60));
    expect(syncGamelist).toHaveBeenCalledTimes(1);
  });

  it("discards stale query results", async () => {
    vi.mocked(syncGamelist).mockReturnValue(new Promise<SyncResult>(() => {}));
    let resolveOld: (value: Awaited<ReturnType<typeof getDatabasePage>>) => void = () => {};
    vi.mocked(getDatabasePage).mockImplementation((query) =>
      query === ""
        ? new Promise((resolve) => {
            resolveOld = resolve;
          })
        : Promise.resolve({
            items: [{ ...row, name: "New results" }],
            offset: 0,
            limit: 60,
            hasMore: false,
          }),
    );
    render(<AddGameDialog open onOpenChange={vi.fn()} onAdd={vi.fn()} />);
    await waitFor(() => expect(getDatabasePage).toHaveBeenCalled());
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "new" } });
    await screen.findByText("New results");
    resolveOld({ items: [row], offset: 0, limit: 60, hasMore: false });
    await waitFor(() => expect(screen.queryByText("Cached game")).toBeNull());
  });

  it("requires confirmation before removing Steam integration", async () => {
    vi.mocked(getSteamLibraries).mockResolvedValue(["C:\\Steam"]);
    const remove = vi.fn().mockResolvedValue(undefined);
    render(
      <SteamDialog
        open
        game={game}
        onOpenChange={vi.fn()}
        onRemove={remove}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "steam.remove" }));
    expect(remove).not.toHaveBeenCalled();
    await screen.findByText("steam.confirmRemove");
    expect(screen.queryByRole("button", { name: "steam.remove" })).toBeNull();
    expect(screen.queryByRole("button", { name: "steam.update" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "steam.remove" }));
    fireEvent.click(screen.getByRole("button", { name: "steam.confirmRemoveAction" }));
    await waitFor(() => expect(remove).toHaveBeenCalledOnce());
  });

  it("opens the manifest and legacy executable folders for inspection", async () => {
    vi.mocked(getSteamLibraries).mockResolvedValue(["C:\\Steam"]);
    render(
      <SteamDialog open game={game} onOpenChange={vi.fn()} onRemove={vi.fn()} onSave={vi.fn()} />,
    );
    expect(await screen.findByText("C:\\Steam\\steamapps\\common\\Dummy\\old.exe")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "steam.openManifestFolder" }));
    fireEvent.click(screen.getByRole("button", { name: "steam.openDummyFolder" }));
    expect(openSteamIntegrationFolder).toHaveBeenNthCalledWith(1, "123", "game.exe", true);
    expect(openSteamIntegrationFolder).toHaveBeenNthCalledWith(2, "123", "game.exe", false);
  });

  it("list view can open, launch and favorite a game", () => {
    const open = vi.fn();
    const launch = vi.fn();
    const favorite = vi.fn();
    render(
      <GameList
        games={[game]}
        busyKey={null}
        onOpen={open}
        onToggleRunning={launch}
        onToggleFavorite={favorite}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Example" }));
    fireEvent.click(screen.getByRole("button", { name: "game.play" }));
    fireEvent.click(screen.getByRole("button", { name: "game.favorite" }));
    expect(open).toHaveBeenCalledWith(game);
    expect(launch).toHaveBeenCalledWith(game);
    expect(favorite).toHaveBeenCalledWith(game);
  });
});
