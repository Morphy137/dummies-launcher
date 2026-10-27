// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { Game } from "../types";
import { resolveSteamArtwork } from "./backend";
import { useGameBannerCandidates } from "./useGameBannerCandidates";

vi.mock("./backend", () => ({ resolveSteamArtwork: vi.fn() }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

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

it("uses the gradient when artwork services are unreachable", async () => {
  vi.mocked(resolveSteamArtwork).mockRejectedValue(new Error("offline"));
  const hook = renderHook(() => useGameBannerCandidates({ ...game, name: "Offline Game" }));
  await act(async () => {});
  expect(hook.result.current).toEqual([]);
});

it("ignores stale responses and clears the previous game's artwork immediately", async () => {
  let finish: (urls: string[]) => void = () => {};
  vi.mocked(resolveSteamArtwork)
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValueOnce(["aion-banner"])
    .mockReturnValueOnce(new Promise(() => {}));
  const hook = renderHook(({ selected }) => useGameBannerCandidates(selected), {
    initialProps: { selected: game },
  });
  hook.rerender({ selected: { ...game, name: "AION 2" } });
  await waitFor(() => expect(hook.result.current[0]).toBe("aion-banner"));
  await act(async () => finish(["stale-aniimo-banner"]));
  expect(hook.result.current).not.toContain("stale-aniimo-banner");
  hook.rerender({ selected: { ...game, name: "ARKNIGHTS: ENDFIELD" } });
  expect(hook.result.current).not.toContain("aion-banner");
});
