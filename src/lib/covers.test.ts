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

  it("prefers Steam heroes, then the landscape header", () => {
    const candidates = getGameBannerCandidates(game);
    expect(candidates).toHaveLength(3);
    expect(candidates[0]).toContain("/730/library_hero_2x.jpg");
    expect(candidates[1]).toContain("/730/library_hero.jpg");
    expect(candidates[2]).toContain("/730/header.jpg");
  });

  it.each([null, "", "../730", "730?key=example"])(
    "uses no banner when Steam metadata is absent or invalid: %s",
    (steamAppId) => {
      expect(getGameBannerCandidates({ ...game, steamAppId })).toEqual([]);
    },
  );

  it("does not enlarge a small icon into a banner when no cover is available", () => {
    expect(getGameBannerCandidates({ ...game, steamAppId: null, coverImageHash: null })).toEqual(
      [],
    );
  });
});
