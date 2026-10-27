// @vitest-environment jsdom
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { resolveSteamArtwork } from "../lib/backend";
import { getGameBannerCandidates } from "../lib/covers";
import type { Game } from "../types";
import GameDetail from "./GameDetail";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("../lib/backend", () => ({
  resolveSteamArtwork: vi.fn().mockResolvedValue([]),
  resolveSteamCover: vi.fn().mockResolvedValue([]),
}));
vi.mock("../lib/coverCache", () => ({
  getCachedCoverUrl: () => undefined,
  getCoverUrl: async (url: string) => url,
  retainCover: () => () => {},
}));
afterEach(cleanup);

const game: Game = {
  appId: "1461154307171811401",
  name: "ARKNIGHTS: ENDFIELD",
  exe: "endfield.exe",
  coverImageHash: "cover",
  running: false,
  isFavorite: false,
  requiresSteam: false,
  useSteamPath: false,
};

describe("detail banner", () => {
  it("loads current Steam artwork by name even without a catalog Steam ID", async () => {
    const url =
      "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/4732690/hash/page_bg_raw.jpg";
    vi.mocked(resolveSteamArtwork).mockResolvedValueOnce([url]);
    const { container } = render(
      <GameDetail
        game={game}
        busy={false}
        canPause={false}
        onBack={vi.fn()}
        onConfigureSteam={vi.fn()}
        onCreateShortcut={vi.fn()}
        onDelete={vi.fn()}
        onToggleFavorite={vi.fn()}
        onToggleRunning={vi.fn()}
        onTogglePause={vi.fn()}
        onUpdateExe={vi.fn()}
      />,
    );
    await waitFor(() =>
      expect(container.querySelector(".detail__banner")?.getAttribute("src")).toBe(url),
    );
    expect(resolveSteamArtwork).toHaveBeenLastCalledWith(game.name, undefined);
    expect(game.steamAppId).toBeUndefined();
  });
  it.each([null, "730"])(
    "renders the gradient after any Steam artwork fails (Steam ID: %s)",
    async (steamAppId) => {
      const selected = { ...game, steamAppId };
      const { container } = render(
        <GameDetail
          game={selected}
          busy={false}
          canPause={false}
          onBack={vi.fn()}
          onConfigureSteam={vi.fn()}
          onCreateShortcut={vi.fn()}
          onDelete={vi.fn()}
          onToggleFavorite={vi.fn()}
          onToggleRunning={vi.fn()}
          onTogglePause={vi.fn()}
          onUpdateExe={vi.fn()}
        />,
      );
      const candidates = getGameBannerCandidates(selected);
      for (const candidate of candidates) {
        await waitFor(() =>
          expect(container.querySelector(".detail__banner")?.getAttribute("src")).toBe(candidate),
        );
        fireEvent.error(container.querySelector(".detail__banner") as HTMLImageElement);
      }
      await waitFor(() => expect(container.querySelector(".detail__banner")).toBeNull());
    },
  );
});
