import {
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Spinner,
} from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getSteamPath } from "../lib/backend";
import type { Game, SteamSetupRequest } from "../types";

interface SteamDialogProps {
  open: boolean;
  game: Game | null;
  onOpenChange: (open: boolean) => void;
  onRemove: () => Promise<void>;
  onSave: (request: SteamSetupRequest) => Promise<void>;
}

function defaultInstallDir(game: Game): string {
  return (
    game.installDir ||
    game.name
      .replace(/[<>:"/\\|?*]/g, "_")
      .trim()
      .slice(0, 120)
  );
}

export default function SteamDialog({
  open,
  game,
  onOpenChange,
  onRemove,
  onSave,
}: SteamDialogProps) {
  const { t } = useTranslation();
  const [steamPath, setSteamPath] = useState<string | null>(null);
  const [steamAppId, setSteamAppId] = useState("");
  const [installDir, setInstallDir] = useState("");
  const [exe, setExe] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !game) return;

    let cancelled = false;
    setError(null);
    setSteamAppId(game.steamAppId || "");
    setInstallDir(defaultInstallDir(game));
    setExe(game.exe);

    if (game.name.toLowerCase().includes("tokon")) {
      setSteamAppId((current) => current || "3787240");
      setInstallDir((current) => current || "MTFS");
      setExe((current) =>
        current.includes("MTFSSteam") ? current : "Binaries/Win64/MTFSSteam-Win64-Shipping.exe",
      );
    }

    void getSteamPath().then((path) => {
      if (!cancelled) setSteamPath(path);
    });

    return () => {
      cancelled = true;
    };
  }, [game, open]);

  const handleSave = async () => {
    if (!game) return;
    setBusy(true);
    setError(null);
    try {
      await onSave({
        appId: game.appId,
        steamAppId: steamAppId.trim(),
        installDir: installDir.trim(),
        exe: exe.trim(),
      });
    } catch (saveError: unknown) {
      setError(saveError instanceof Error ? saveError.message : String(saveError));
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = async () => {
    setBusy(true);
    setError(null);
    try {
      await onRemove();
    } catch (removeError: unknown) {
      setError(removeError instanceof Error ? removeError.message : String(removeError));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(_, data) => onOpenChange(data.open)}>
      <DialogSurface className="steam-dialog">
        <DialogBody className="steam-body">
          <DialogTitle className="steam-title">{t("steam.title")}</DialogTitle>
          <DialogContent className="steam-content">
            <p className="steam-note">{t("steam.note")}</p>
            <div className="steam-path">
              <span>{t("steam.path")}</span>
              <span className="steam-path__value">{steamPath || t("steam.notFound")}</span>
            </div>

            <label className="steam-field">
              <span>{t("steam.appId")}</span>
              <input
                className="steam-input"
                value={steamAppId}
                onChange={(event) => setSteamAppId(event.target.value)}
                placeholder="3787240"
                spellCheck={false}
              />
            </label>

            <label className="steam-field">
              <span>{t("steam.installDir")}</span>
              <input
                className="steam-input"
                value={installDir}
                onChange={(event) => setInstallDir(event.target.value)}
                placeholder="MTFS"
                spellCheck={false}
              />
            </label>

            <label className="steam-field">
              <span>{t("steam.exe")}</span>
              <input
                className="steam-input"
                value={exe}
                onChange={(event) => setExe(event.target.value)}
                placeholder="Binaries/Win64/Game.exe"
                spellCheck={false}
              />
            </label>

            {error ? <div className="catalog-error">{error}</div> : null}
          </DialogContent>

          <DialogActions className="steam-actions">
            {game?.useSteamPath ? (
              <button
                type="button"
                className="secondary-button detail__danger-button"
                onClick={() => void handleRemove()}
                disabled={busy}
              >
                {t("steam.remove")}
              </button>
            ) : null}
            <span className="steam-actions__spacer" />
            <button
              type="button"
              className="secondary-button"
              onClick={() => onOpenChange(false)}
              disabled={busy}
            >
              {t("common.cancel")}
            </button>
            <button
              type="button"
              className="primary-button"
              onClick={() => void handleSave()}
              disabled={busy || !steamPath}
            >
              {busy ? <Spinner size="tiny" /> : null}
              {game?.useSteamPath ? t("steam.update") : t("steam.install")}
            </button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
