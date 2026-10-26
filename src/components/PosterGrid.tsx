import { type CSSProperties, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type Game, makeGameKey } from "../types";
import PosterCard from "./PosterCard";

interface PosterGridProps {
  games: Game[];
  cardsPerRow: number;
  maxCardWidth: number;
  canPause: boolean;
  onOpenGame: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onToggleRunning: (game: Game) => void;
  onTogglePause: (game: Game) => void;
  busyKey: string | null;
}

export default function PosterGrid({
  games,
  cardsPerRow,
  maxCardWidth,
  canPause,
  onOpenGame,
  onToggleFavorite,
  onToggleRunning,
  onTogglePause,
  busyKey,
}: PosterGridProps) {
  const { t } = useTranslation();
  const gridRef = useRef<HTMLElement>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    if (!gridRef.current) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    observer.observe(gridRef.current);
    return () => observer.disconnect();
  }, []);
  const columns = width
    ? Math.min(cardsPerRow, Math.max(1, Math.floor((width + 16) / 146)))
    : cardsPerRow;
  const gridStyle = {
    "--cards-per-row": String(columns),
    "--card-max-width": `${maxCardWidth}px`,
  } as CSSProperties;

  return (
    <section
      ref={gridRef}
      className="poster-grid"
      style={gridStyle}
      aria-label={t("library.title")}
      onKeyDown={(event) => {
        const offsets: Record<string, number> = {
          ArrowRight: 1,
          ArrowLeft: -1,
          ArrowDown: columns,
          ArrowUp: -columns,
        };
        const offset = offsets[event.key];
        if (offset === undefined || !(event.target instanceof HTMLElement)) return;
        const card = event.target.closest(".poster-card");
        const cards = Array.from(event.currentTarget.querySelectorAll<HTMLElement>(".poster-card"));
        const index = card instanceof HTMLElement ? cards.indexOf(card) : -1;
        const next = cards[index + offset]?.querySelector<HTMLButtonElement>(".poster-card__open");
        if (next) {
          event.preventDefault();
          next.focus();
        }
      }}
    >
      {games.map((game) => (
        <PosterCard
          key={makeGameKey(game)}
          game={game}
          busy={busyKey === makeGameKey(game)}
          canPause={canPause}
          onOpen={onOpenGame}
          onToggleFavorite={onToggleFavorite}
          onToggleRunning={onToggleRunning}
          onTogglePause={onTogglePause}
        />
      ))}
    </section>
  );
}
