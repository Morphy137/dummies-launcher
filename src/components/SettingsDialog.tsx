import {
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
} from "@fluentui/react-components";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { checkForUpdates } from "../lib/backend";
import type { LanguageMode, Settings, ThemeMode, UpdateInfo } from "../types";

interface SettingsDialogProps {
  open: boolean;
  settings: Settings;
  version: string;
  onPreview: (settings: Settings) => void;
  onCancel: () => void;
  onDismiss: () => void;
  onSave: (settings: Settings) => Promise<void>;
}

interface ToggleRowProps {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

function ToggleRow({ label, description, checked, onChange }: ToggleRowProps) {
  return (
    <label className="settings-row">
      <span className="settings-row__text">
        <span className="settings-row__label">{label}</span>
        <span className="settings-row__desc">{description}</span>
      </span>
      <span className="switch">
        <input
          type="checkbox"
          checked={checked}
          onChange={(event) => onChange(event.target.checked)}
        />
        <span className="switch__track">
          <span className="switch__thumb" />
        </span>
      </span>
    </label>
  );
}

export default function SettingsDialog({
  open,
  settings,
  version,
  onPreview,
  onCancel,
  onDismiss,
  onSave,
}: SettingsDialogProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<Settings>(settings);
  const [saving, setSaving] = useState(false);
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);

  useEffect(() => {
    if (open) setDraft(settings);
  }, [open, settings]);

  const updateDraft = (next: Settings) => {
    setDraft(next);
    onPreview(next);
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await onSave(draft);
    } finally {
      setSaving(false);
    }
  };

  const handleCheckUpdates = async () => {
    setCheckingUpdates(true);
    setUpdateError(null);
    try {
      setUpdateInfo(await checkForUpdates());
    } catch (error: unknown) {
      setUpdateError(error instanceof Error ? error.message : String(error));
    } finally {
      setCheckingUpdates(false);
    }
  };

  const openRelease = async () => {
    if (!updateInfo?.releaseUrl) return;
    try {
      await openUrl(updateInfo.releaseUrl);
    } catch (error: unknown) {
      setUpdateError(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(_, data) => {
        if (data.open) return;
        if (data.type === "backdropClick" || data.type === "escapeKeyDown") {
          onDismiss();
        }
      }}
    >
      <DialogSurface className="settings-dialog">
        <DialogBody className="settings-body">
          <DialogTitle className="settings-title">{t("settings.title")}</DialogTitle>
          <DialogContent className="settings-content">
            <section className="settings-section">
              <div className="settings-section-title">{t("settings.appearance")}</div>

              <div className="settings-row">
                <span className="settings-row__text">
                  <span className="settings-row__label">{t("settings.theme")}</span>
                </span>
                <select
                  className="settings-select"
                  value={draft.theme}
                  onChange={(event) =>
                    updateDraft({ ...draft, theme: event.target.value as ThemeMode })
                  }
                  aria-label={t("settings.theme")}
                >
                  <option value="system">{t("settings.themeSystem")}</option>
                  <option value="dark">{t("settings.themeDark")}</option>
                  <option value="light">{t("settings.themeLight")}</option>
                </select>
              </div>

              <div className="settings-row">
                <span className="settings-row__text">
                  <span className="settings-row__label">{t("settings.language")}</span>
                </span>
                <select
                  className="settings-select"
                  value={draft.language}
                  onChange={(event) =>
                    updateDraft({ ...draft, language: event.target.value as LanguageMode })
                  }
                  aria-label={t("settings.language")}
                >
                  <option value="system">{t("settings.languageSystem")}</option>
                  <option value="en">{t("settings.languageEnglish")}</option>
                  <option value="es">{t("settings.languageSpanish")}</option>
                </select>
              </div>

              <div className="settings-row">
                <span className="settings-row__text">
                  <span className="settings-row__label">{t("settings.cardsPerRow")}</span>
                  <span className="settings-row__desc">{t("settings.cardsPerRowDesc")}</span>
                </span>
                <select
                  className="settings-select"
                  value={draft.cardsPerRow}
                  onChange={(event) =>
                    updateDraft({ ...draft, cardsPerRow: Number(event.target.value) })
                  }
                  aria-label={t("settings.cardsPerRow")}
                >
                  {[4, 5, 6, 7, 8].map((count) => (
                    <option key={count} value={count}>
                      {t("settings.cardsPerRowValue", { count })}
                    </option>
                  ))}
                </select>
              </div>
            </section>

            <section className="settings-section">
              <div className="settings-section-title">{t("settings.questAutomation")}</div>

              <ToggleRow
                label={t("settings.questTimer")}
                description={t("settings.questTimerDesc")}
                checked={draft.questTimerEnabled}
                onChange={(questTimerEnabled) => updateDraft({ ...draft, questTimerEnabled })}
              />

              <div className="settings-row">
                <span className="settings-row__text">
                  <span className="settings-row__label">{t("settings.questDuration")}</span>
                  <span className="settings-row__desc">{t("settings.questDurationDesc")}</span>
                </span>
                <select
                  className="settings-select"
                  value={draft.questDurationMinutes}
                  onChange={(event) =>
                    updateDraft({
                      ...draft,
                      questDurationMinutes: Number(event.target.value),
                    })
                  }
                  aria-label={t("settings.questDuration")}
                  disabled={!draft.questTimerEnabled}
                >
                  {[10, 15, 20, 30].map((minutes) => (
                    <option key={minutes} value={minutes}>
                      {t("settings.minutesValue", { count: minutes })}
                    </option>
                  ))}
                </select>
              </div>

              <div className="settings-row">
                <span className="settings-row__text">
                  <span className="settings-row__label">{t("settings.questSyncBuffer")}</span>
                  <span className="settings-row__desc">{t("settings.questSyncBufferDesc")}</span>
                </span>
                <select
                  className="settings-select"
                  value={draft.questSyncBufferSeconds}
                  onChange={(event) =>
                    updateDraft({
                      ...draft,
                      questSyncBufferSeconds: Number(event.target.value),
                    })
                  }
                  aria-label={t("settings.questSyncBuffer")}
                  disabled={!draft.questTimerEnabled}
                >
                  {[0, 30, 60, 90].map((seconds) => (
                    <option key={seconds} value={seconds}>
                      {seconds === 0
                        ? t("settings.off")
                        : t("settings.secondsValue", { count: seconds })}
                    </option>
                  ))}
                </select>
              </div>

              <ToggleRow
                label={t("settings.autoStop")}
                description={t("settings.autoStopDesc")}
                checked={draft.autoStopOnComplete}
                onChange={(autoStopOnComplete) => updateDraft({ ...draft, autoStopOnComplete })}
              />

              <ToggleRow
                label={t("settings.notify")}
                description={t("settings.notifyDesc")}
                checked={draft.notifyOnComplete}
                onChange={(notifyOnComplete) => updateDraft({ ...draft, notifyOnComplete })}
              />
            </section>

            <section className="settings-section">
              <div className="settings-section-title">{t("settings.dummyProcess")}</div>

              <ToggleRow
                label={t("settings.showDummyWindow")}
                description={t("settings.showDummyWindowDesc")}
                checked={draft.showDummyWindow}
                onChange={(showDummyWindow) => updateDraft({ ...draft, showDummyWindow })}
              />

              <ToggleRow
                label={t("settings.minimizeToTray")}
                description={t("settings.minimizeToTrayDesc")}
                checked={draft.minimizeToTray}
                onChange={(minimizeToTray) => updateDraft({ ...draft, minimizeToTray })}
              />
            </section>

            <section className="settings-section">
              <div className="settings-section-title">{t("settings.updates")}</div>

              <div className="settings-row">
                <span className="settings-row__text">
                  <span className="settings-row__label">{t("settings.version")}</span>
                  <span className="settings-row__desc">v{version}</span>
                </span>
                <button
                  type="button"
                  className="secondary-button"
                  onClick={() => void handleCheckUpdates()}
                  disabled={checkingUpdates}
                >
                  {checkingUpdates ? t("settings.checking") : t("settings.checkUpdates")}
                </button>
              </div>

              {updateInfo ? (
                <div className="settings-update-status">
                  <span>
                    {updateInfo.updateAvailable
                      ? t("settings.updateAvailable", { version: updateInfo.latestVersion })
                      : t("settings.upToDate")}
                  </span>
                  {updateInfo.updateAvailable ? (
                    <button
                      type="button"
                      className="secondary-button"
                      onClick={() => void openRelease()}
                    >
                      {t("settings.openRelease")}
                    </button>
                  ) : null}
                </div>
              ) : null}

              {updateError ? <div className="catalog-error">{updateError}</div> : null}
            </section>

            <p className="settings-note">{t("settings.note")}</p>
          </DialogContent>

          <DialogActions className="settings-actions">
            <button type="button" className="secondary-button" onClick={onCancel} disabled={saving}>
              {t("common.cancel")}
            </button>
            <button
              type="button"
              className="primary-button"
              onClick={() => void handleSave()}
              disabled={saving}
            >
              {t("common.save")}
            </button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
