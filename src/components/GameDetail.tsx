import {
  ArrowLeft20Regular,
  Delete20Regular,
  Edit20Regular,
  Heart20Filled,
  Heart20Regular,
  Image20Regular,
  Open20Regular,
  Pause20Filled,
  Play20Filled,
  Settings20Regular,
  Stop20Filled,
} from "@fluentui/react-icons";
import { type CSSProperties, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getFallbackArt,
  getGameBannerCandidates,
  getGameCoverCandidates,
  getGameInitials,
} from "../lib/covers";
import { useCoverCandidates } from "../lib/useCoverCandidates";
import type { Game } from "../types";
import RetroClock from "./RetroClock";

interface GameDetailProps {
  game: Game;
  busy: boolean;
  canPause: boolean;
  onBack: () => void;
  onConfigureSteam: (game: Game) => void;
  onCreateShortcut: (game: Game) => Promise<void>;
  onDelete: (game: Game) => Promise<void>;
  onToggleFavorite: (game: Game) => void;
  onToggleRunning: (game: Game) => void;
  onTogglePause: (game: Game) => void;
  onUpdateExe: (game: Game, exe: string) => Promise<void>;
}

export default function GameDetail({
  game,
  busy,
  canPause,
  onBack,
  onConfigureSteam,
  onCreateShortcut,
  onDelete,
  onToggleFavorite,
  onToggleRunning,
  onTogglePause,
  onUpdateExe,
}: GameDetailProps) {
  const { t } = useTranslation();
  const [editingExe, setEditingExe] = useState(false);
  const [exeDraft, setExeDraft] = useState(game.exe);
  const exeInputRef = useRef<HTMLInputElement>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const backRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    backRef.current?.focus();
  }, []);

  const candidates = getGameCoverCandidates(game);
  const {
    src: cachedCover,
    failed: coverFailed,
    handleError: handleCoverError,
    handleLoad: handleCoverLoad,
  } = useCoverCandidates(candidates);
  const bannerCandidates = getGameBannerCandidates(game);
  const {
    src: banner,
    failed: bannerFailed,
    handleError: handleBannerError,
    handleLoad: handleBannerLoad,
  } = useCoverCandidates(bannerCandidates);
  const fallback = getFallbackArt(game.name);
  const initials = getGameInitials(game.name);
  const steamStatus = game.useSteamPath
    ? t("detail.steamConfigured")
    : game.requiresSteam
      ? t("detail.steamRequired")
      : game.steamAppId
        ? t("detail.steamOptional")
        : t("detail.steamUnavailable");

  useEffect(() => {
    if (editingExe) {
      exeInputRef.current?.focus();
      exeInputRef.current?.select();
    }
  }, [editingExe]);

  const startEditingExe = () => {
    setExeDraft(game.exe);
    setEditingExe(true);
  };

  const saveExe = async () => {
    const next = exeDraft.trim();
    if (!next || next === game.exe) {
      setEditingExe(false);
      return;
    }
    try {
      await onUpdateExe(game, next);
      setEditingExe(false);
    } catch {
      // The parent reports the error through the toast.
    }
  };

  const deleteGame = async () => {
    if (game.running || game.useSteamPath) return;
    setDeleting(true);
    try {
      await onDelete(game);
    } catch {
      // The parent reports the error through the toast.
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="detail">
      <button ref={backRef} type="button" className="detail__back" onClick={onBack}>
        <ArrowLeft20Regular />
        {t("detail.back")}
      </button>

      <section
        className="detail__hero"
        style={
          {
            "--art-a": fallback[0],
            "--art-b": fallback[1],
          } as CSSProperties
        }
      >
        {banner && !bannerFailed ? (
          <img
            className="detail__banner"
            src={banner}
            alt=""
            onError={handleBannerError}
            onLoad={handleBannerLoad}
          />
        ) : null}
        <div className="detail__scrim" />
        <div className="detail__hero-content">
          <div className="detail__poster">
            {cachedCover && !coverFailed ? (
              <img src={cachedCover} alt="" onError={handleCoverError} onLoad={handleCoverLoad} />
            ) : (
              <span className="poster-card__placeholder" aria-hidden="true">
                <Image20Regular />
                <span className="poster-card__initials">{initials}</span>
              </span>
            )}
          </div>

          <div className="detail__info">
            {game.running ? (
              <div className="detail__timer">
                <RetroClock time={game.remaining} paused={game.paused} large />
              </div>
            ) : null}
            <h1>{game.name}</h1>
            <div className="detail__meta">
              <span>{game.exe}</span>
              <span>{t("detail.appId", { id: game.appId })}</span>
            </div>
            <div className="detail__actions">
              <button
                type="button"
                className={`primary-button${game.running ? " is-danger" : ""}`}
                onClick={() => onToggleRunning(game)}
                disabled={busy}
              >
                {game.running ? <Stop20Filled /> : <Play20Filled />}
                {game.running ? t("game.stop") : t("game.play")}
              </button>
              {game.running && canPause && game.phase && game.phase !== "complete" ? (
                <button
                  type="button"
                  className="secondary-button detail__pause"
                  onClick={() => onTogglePause(game)}
                  disabled={busy}
                >
                  {game.paused ? <Play20Filled /> : <Pause20Filled />}
                  {game.paused ? t("game.resume") : t("game.pause")}
                </button>
              ) : null}
              <button
                type="button"
                className={`detail__favorite${game.isFavorite ? " is-favorite" : ""}`}
                onClick={() => onToggleFavorite(game)}
                aria-label={game.isFavorite ? t("game.unfavorite") : t("game.favorite")}
                title={game.isFavorite ? t("game.unfavorite") : t("game.favorite")}
              >
                {game.isFavorite ? <Heart20Filled /> : <Heart20Regular />}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section className="detail__panel">
        <div className="settings-section-title">{t("detail.details")}</div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.appIdLabel")}</span>
          <span className="detail__value">{game.appId}</span>
        </div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.executable")}</span>
          {editingExe ? (
            <div className="detail__exe-edit">
              <input
                aria-label={t("detail.executable")}
                ref={exeInputRef}
                className="detail__input"
                value={exeDraft}
                onChange={(event) => setExeDraft(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void saveExe();
                  if (event.key === "Escape") setEditingExe(false);
                }}
                spellCheck={false}
              />
              <button type="button" className="secondary-button" onClick={() => void saveExe()}>
                {t("common.save")}
              </button>
              <button
                type="button"
                className="secondary-button"
                onClick={() => setEditingExe(false)}
              >
                {t("common.cancel")}
              </button>
            </div>
          ) : (
            <div className="detail__value-row">
              <span className="detail__value detail__mono">{game.exe}</span>
              <button
                type="button"
                className="icon-button icon-button--small"
                onClick={startEditingExe}
                disabled={game.running || game.useSteamPath}
                aria-label={t("detail.editExe")}
                title={t("detail.editExe")}
              >
                <Edit20Regular />
              </button>
            </div>
          )}
        </div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.running")}</span>
          <span className="detail__value">{game.running ? t("detail.yes") : t("detail.no")}</span>
        </div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.steam")}</span>
          <div className="detail__value-row">
            <span
              className={`detail__value${game.requiresSteam && !game.useSteamPath ? " is-warning" : ""}`}
            >
              {steamStatus}
            </span>
            {game.steamAppId || game.requiresSteam || game.useSteamPath ? (
              <button
                type="button"
                className="icon-button icon-button--small"
                onClick={() => onConfigureSteam(game)}
                aria-label={t("steam.title")}
                title={t("steam.title")}
              >
                <Settings20Regular />
              </button>
            ) : null}
          </div>
        </div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.shortcut")}</span>
          <button
            type="button"
            className="secondary-button"
            onClick={() => void onCreateShortcut(game)}
          >
            <Open20Regular />
            {t("detail.createShortcut")}
          </button>
        </div>

        <div className="detail__danger">
          {game.useSteamPath ? <p>{t("steam.removeBeforeDelete")}</p> : null}
          {confirmDelete ? (
            <div className="detail__confirm">
              <span>{t("detail.confirmDelete", { name: game.name })}</span>
              <button
                type="button"
                className="secondary-button detail__danger-button"
                onClick={() => void deleteGame()}
                disabled={deleting || game.running || game.useSteamPath}
              >
                {deleting ? t("detail.deleting") : t("detail.delete")}
              </button>
              <button
                type="button"
                className="secondary-button"
                onClick={() => setConfirmDelete(false)}
                disabled={deleting}
              >
                {t("common.cancel")}
              </button>
            </div>
          ) : (
            <button
              type="button"
              className="secondary-button detail__danger-button"
              onClick={() => setConfirmDelete(true)}
              disabled={game.running || game.useSteamPath}
            >
              <Delete20Regular />
              {t("detail.delete")}
            </button>
          )}
        </div>
      </section>
    </div>
  );
}
