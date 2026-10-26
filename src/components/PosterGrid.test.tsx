// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Game } from "../types";
import PosterGrid from "./PosterGrid";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("./PosterCard", () => ({ default: ({ game }: { game: Game }) => <div>{game.name}</div> }));
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("bounded poster grid", () => {
  it.each([
    [3840, 24],
    [940, 6],
  ])("fits the requested columns into %i px", (width, columns) => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(private callback: ResizeObserverCallback) {}
        observe() {
          this.callback(
            [{ contentRect: { width } }] as ResizeObserverEntry[],
            this as unknown as ResizeObserver,
          );
        }
        unobserve() {}
        disconnect() {}
      },
    );
    const props = {
      games: [],
      cardsPerRow: 24,
      maxCardWidth: 180,
      canPause: true,
      onOpenGame: vi.fn(),
      onToggleFavorite: vi.fn(),
      onToggleRunning: vi.fn(),
      onTogglePause: vi.fn(),
      busyKey: null,
    };
    const { rerender } = render(<PosterGrid {...props} />);
    const grid = screen.getByRole("region", { name: "library.title" });
    expect(grid.style.getPropertyValue("--cards-per-row")).toBe(String(columns));
    expect(grid.style.getPropertyValue("--card-max-width")).toBe("180px");
    rerender(<PosterGrid {...props} maxCardWidth={240} />);
    expect(grid.style.getPropertyValue("--card-max-width")).toBe("240px");
  });
});
