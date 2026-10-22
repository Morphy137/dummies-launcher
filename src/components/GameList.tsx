import { Heart20Filled, Heart20Regular } from "@fluentui/react-icons";
import { useTranslation } from "react-i18next";
import { type Game, makeGameKey } from "../types";

interface GameListProps {
  games: Game[];
}

export default function GameList({ games }: GameListProps) {
  const { t } = useTranslation();

  return (
    <section className="game-list" aria-label={t("library.title")}>
      {games.map((game) => (
        <div key={makeGameKey(game)} className={`game-row${game.running ? " is-running" : ""}`}>
          <span className={`status-dot${game.running ? " is-active" : ""}`} aria-hidden="true" />
          <span className="game-row__name">{game.name}</span>
          <span className="game-row__exe">{game.exe}</span>
          <span className="game-row__state">
            {game.running
              ? `${t("game.running")}${game.remaining ? ` · ${game.remaining}` : ""}`
              : ""}
          </span>
          <span
            className={`game-row__favorite${game.isFavorite ? " is-active" : ""}`}
            aria-hidden="true"
          >
            {game.isFavorite ? <Heart20Filled /> : <Heart20Regular />}
          </span>
        </div>
      ))}
    </section>
  );
}
