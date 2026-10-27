// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Game } from "../types";
import GameDetail from "./GameDetail";
import PosterCard from "./PosterCard";
import RetroClock from "./RetroClock";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("../lib/useCoverCandidates", () => ({
  useCoverCandidates: () => ({ src: undefined, failed: true, handleError: vi.fn() }),
}));

afterEach(cleanup);

const game: Game = {
  appId: "123",
  name: "Example",
  exe: "game.exe",
  isFavorite: false,
  running: true,
  remaining: "13:20",
  phase: "playing",
  percent: 10,
  requiresSteam: false,
  useSteamPath: false,
};

describe("seven-segment clock", () => {
  it.each([
    ["0", 6],
    ["1", 2],
    ["2", 5],
    ["3", 5],
    ["4", 4],
    ["5", 5],
    ["6", 6],
    ["7", 3],
    ["8", 7],
    ["9", 6],
    ["-", 1],
  ] as const)("lights the correct segments for %s", (time, count) => {
    const { container } = render(<RetroClock time={time} />);
    expect(screen.getByRole("timer", { name: time })).toBeDefined();
    expect(container.querySelectorAll(".retro-clock__segment")).toHaveLength(7);
    expect(container.querySelectorAll(".retro-clock__segment.is-lit")).toHaveLength(count);
  });

  it("supports three-digit minutes without truncating the display", () => {
    const { container } = render(<RetroClock time="120:00" paused large />);
    expect(screen.getByRole("timer", { name: "120:00" }).className).toContain("is-paused");
    expect(container.querySelectorAll(".retro-clock__segment")).toHaveLength(35);
    expect(container.querySelectorAll(".retro-clock__colon")).toHaveLength(2);
    expect(container.querySelector("svg")?.getAttribute("aria-hidden")).toBe("true");
  });

  it("renders dashes until the first timer snapshot arrives", () => {
    const { container } = render(<RetroClock />);
    expect(container.querySelectorAll(".retro-clock__segment.is-lit")).toHaveLength(4);
  });
});

describe("game timer presentation", () => {
  it("uses the shared compact clock inside the poster card", () => {
    render(
      <PosterCard
        game={game}
        busy={false}
        canPause
        onOpen={vi.fn()}
        onToggleFavorite={vi.fn()}
        onToggleRunning={vi.fn()}
        onTogglePause={vi.fn()}
      />,
    );
    const timer = screen.getByRole("timer", { name: "13:20" });
    expect(timer.closest(".poster-card__art")).not.toBeNull();
    expect(timer.className).not.toContain("retro-clock--large");
    expect(screen.getByRole("button", { name: "game.pause" })).toBeDefined();
    expect(screen.getByRole("button", { name: "game.stop" })).toBeDefined();
  });

  it("shows only the large clock above the game title, without a phase or progress dock", () => {
    const { container } = render(
      <GameDetail
        game={game}
        busy={false}
        canPause
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
    expect(screen.getByRole("timer", { name: "13:20" }).className).toContain("retro-clock--large");
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.queryByText("game.playing")).toBeNull();
    expect(screen.queryByText("game.localTimer")).toBeNull();
    expect(container.querySelector(".quest-dock")).toBeNull();
    expect(screen.getByRole("button", { name: "game.pause" })).toBeDefined();
  });
});
