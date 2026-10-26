// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { Game } from "../types";
import { resolveSteamCover } from "./backend";
import { useGameCoverCandidates } from "./useGameCoverCandidates";

vi.mock("./backend", () => ({ resolveSteamCover: vi.fn() }));
afterEach(cleanup);

const game: Game = {
  appId: "123",
  name: "Aniimo",
  exe: "game.exe",
  coverImageHash: "cover",
  running: false,
  isFavorite: false,
  requiresSteam: false,
  useSteamPath: false,
};

it("prefers the resolved Steam cover and ignores results for a previous game", async () => {
  let finish: (urls: string[]) => void = () => {};
  vi.mocked(resolveSteamCover)
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValueOnce(["steam-cover"]);
  const hook = renderHook(({ selected }) => useGameCoverCandidates(selected), {
    initialProps: { selected: game },
  });
  hook.rerender({ selected: { ...game, name: "Arknights: Endfield" } });
  await waitFor(() => expect(hook.result.current[0]).toBe("steam-cover"));
  await act(async () => finish(["stale-cover"]));
  expect(hook.result.current).not.toContain("stale-cover");
  expect(hook.result.current).toContain(
    "https://cdn.discordapp.com/app-icons/123/cover.png?size=512",
  );
});
