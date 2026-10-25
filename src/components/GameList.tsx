import { Heart20Filled, Heart20Regular, Play20Filled, Stop20Filled } from "@fluentui/react-icons";
import { useTranslation } from "react-i18next";
import { type Game, makeGameKey } from "../types";

interface GameListProps {
  games: Game[];
  busyKey: string | null;
  onOpen: (game: Game) => void;
  onToggleRunning: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
}

export default function GameList({
  games,
  busyKey,
  onOpen,
  onToggleRunning,
  onToggleFavorite,
}: GameListProps) {
  const { t } = useTranslation();
  return (
    <section className="game-list" aria-label={t("library.title")}>
      {games.map((game) => (
        <div key={makeGameKey(game)} className={`game-row${game.running ? " is-running" : ""}`}>
          <span className={`status-dot${game.running ? " is-active" : ""}`} aria-hidden="true" />
          <button type="button" className="game-row__open" onClick={() => onOpen(game)}>
            {game.name}
          </button>
          <span className="game-row__exe">{game.exe}</span>
          <span className="game-row__state">
            {game.running ? `${t(`game.${game.phase ?? "running"}`)} ${game.remaining ?? ""}` : ""}
          </span>
          <div className="game-row__actions">
            <button
              type="button"
              className="icon-button"
              disabled={busyKey === makeGameKey(game)}
              aria-label={t(game.running ? "game.stop" : "game.play")}
              onClick={() => onToggleRunning(game)}
            >
              {game.running ? <Stop20Filled /> : <Play20Filled />}
            </button>
            <button
              type="button"
              className="icon-button"
              aria-pressed={game.isFavorite}
              aria-label={t(game.isFavorite ? "game.unfavorite" : "game.favorite")}
              onClick={() => onToggleFavorite(game)}
            >
              {game.isFavorite ? <Heart20Filled /> : <Heart20Regular />}
            </button>
          </div>
        </div>
      ))}
    </section>
  );
}
