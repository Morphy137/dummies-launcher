import { describe, expect, it } from "vitest";
import type { Game } from "../types";
import { getGameBannerCandidates, getGameCoverCandidates } from "./covers";

const game: Game = {
  appId: "123",
  name: "Example",
  exe: "game.exe",
  steamAppId: "730",
  iconHash: "icon",
  coverImageHash: "cover",
  running: false,
  isFavorite: false,
  requiresSteam: false,
  useSteamPath: false,
};

describe("game artwork", () => {
  it("prefers full-size Steam posters over the half-size capsule", () => {
    const candidates = getGameCoverCandidates(game);
    expect(candidates[0]).toContain("/730/library_600x900_2x.jpg");
    expect(candidates[1]).toContain("/730/library_600x900.jpg");
  });

  it("uses only landscape hero images for detail banners", () => {
    const candidates = getGameBannerCandidates(game);
    expect(candidates).toHaveLength(2);
    expect(candidates[0]).toContain("/730/library_hero_2x.jpg");
    expect(candidates[1]).toContain("/730/library_hero.jpg");
    expect(
      candidates.every(
        (url) =>
          !url.includes("app-icons") && !url.includes("600x900") && !url.includes("header.jpg"),
      ),
    ).toBe(true);
  });

  it.each([null, "", "../730", "730?key=example"])(
    "does not invent a blurred banner for unavailable Steam metadata: %s",
    (steamAppId) => {
      expect(getGameBannerCandidates({ ...game, steamAppId })).toEqual([]);
    },
  );
});
