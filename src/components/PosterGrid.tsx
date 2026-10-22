import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { type Game, makeGameKey } from "../types";
import PosterCard from "./PosterCard";

interface PosterGridProps {
  games: Game[];
  cardsPerRow: number;
  onAddGame: () => void;
  onToggleRunning: (game: Game) => void;
  busyKey: string | null;
}

export default function PosterGrid({
  games,
  cardsPerRow,
  onAddGame,
  onToggleRunning,
  busyKey,
}: PosterGridProps) {
  const { t } = useTranslation();
  const gridStyle = {
    "--cards-per-row": String(cardsPerRow),
  } as CSSProperties;

  if (games.length === 0) {
    return (
      <section className="empty-state">
        <div className="red-stripe" aria-hidden="true" />
        <h2 className="empty-state__title">{t("library.emptyTitle")}</h2>
        <p className="empty-state__body">{t("library.emptyBody")}</p>
        <button type="button" className="primary-button" onClick={onAddGame}>
          {t("library.emptyAction")}
        </button>
      </section>
    );
  }

  return (
    <section className="poster-grid" style={gridStyle} aria-label={t("library.title")}>
      {games.map((game) => (
        <PosterCard
          key={game.id}
          game={game}
          busy={busyKey === makeGameKey(game)}
          onToggleRunning={onToggleRunning}
        />
      ))}
    </section>
  );
}
