import { FluentProvider, webDarkTheme, webLightTheme } from "@fluentui/react-components";
import {
  Add16Filled,
  Grid20Regular,
  List20Regular,
  Search20Regular,
  Settings20Regular,
} from "@fluentui/react-icons";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Update } from "@tauri-apps/plugin-updater";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import AddGameDialog from "./components/AddGameDialog";
import GameDetail from "./components/GameDetail";
import GameList from "./components/GameList";
import PosterGrid from "./components/PosterGrid";
import SettingsDialog from "./components/SettingsDialog";
import SteamDialog from "./components/SteamDialog";
import TitleBar from "./components/TitleBar";
import Toast from "./components/Toast";
import UpdateDialog from "./components/UpdateDialog";
import i18n from "./i18n";
import {
  addGame,
  createShortcut,
  getBootstrap,
  launchDummy,
  onDummyError,
  onDummyExited,
  onDummyStarted,
  onQuestTick,
  pauseDummy,
  removeGame,
  removeSteamIntegration,
  resumeDummy,
  saveSettings,
  setupSteamIntegration,
  stopDummy,
  toggleFavorite,
  updateGameExecutable,
} from "./lib/backend";
import { formatError } from "./lib/errors";
import { checkForUpdate, installUpdate } from "./lib/updater";
import { useResolvedTheme } from "./theme/useResolvedTheme";
import {
  type DatabaseGame,
  DEFAULT_SETTINGS,
  type Game,
  type GameFilter,
  type LibraryGame,
  makeGameKey,
  type QuestTick,
  type Settings,
  type SteamSetupRequest,
} from "./types";

type ViewMode = "grid" | "list";

const filters: GameFilter[] = ["all", "favorites", "active"];

function formatRemaining(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(total / 60);
  const rest = total % 60;
  return `${String(minutes).padStart(2, "0")}:${String(rest).padStart(2, "0")}`;
}

const darkFluentTheme = {
  ...webDarkTheme,
  colorCompoundBrandBackground: "#ffffff",
  colorCompoundBrandBackgroundHover: "#e8e8e8",
  colorCompoundBrandBackgroundPressed: "#e8e8e8",
  colorNeutralForegroundOnBrand: "#000000",
};

const lightFluentTheme = {
  ...webLightTheme,
  colorCompoundBrandBackground: "#111111",
  colorCompoundBrandBackgroundHover: "#2a2a2a",
  colorCompoundBrandBackgroundPressed: "#2a2a2a",
  colorNeutralForegroundOnBrand: "#ffffff",
};

export default function App() {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<GameFilter>("all");
  const [view, setView] = useState<ViewMode>("grid");
  const [library, setLibrary] = useState<LibraryGame[]>([]);
  const [portable, setPortable] = useState(false);
  const [ready, setReady] = useState(false);
  const [bootstrapError, setBootstrapError] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const openedFrom = useRef<HTMLElement | null>(null);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [steamGameKey, setSteamGameKey] = useState<string | null>(null);
  const [addGameOpen, setAddGameOpen] = useState(false);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [settingsSnapshot, setSettingsSnapshot] = useState<Settings>(DEFAULT_SETTINGS);
  const [version, setVersion] = useState("1.0.0");
  const [systemLocale, setSystemLocale] = useState("en");
  const [runningKeys, setRunningKeys] = useState<Set<string>>(() => new Set());
  const runningRef = useRef(runningKeys);
  const [questTicks, setQuestTicks] = useState<Record<string, QuestTick>>({});
  const pauseIntentRef = useRef<Record<string, { desired: boolean; at: number }>>({});
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [toast, setToast] = useState<{
    id: number;
    text: string;
    actionLabel?: string;
    duration?: number;
    onAction?: () => void;
  } | null>(null);
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateOpen, setUpdateOpen] = useState(false);
  const [installingUpdate, setInstallingUpdate] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<number | undefined>();
  const [updateError, setUpdateError] = useState<string | null>(null);

  useEffect(
    () => () => {
      if (availableUpdate) void availableUpdate.close().catch(() => {});
    },
    [availableUpdate],
  );

  const resolvedTheme = useResolvedTheme(settings.theme);
  const language =
    settings.language === "system"
      ? systemLocale.toLowerCase().startsWith("es")
        ? "es"
        : "en"
      : settings.language;

  useEffect(() => {
    let cancelled = false;

    getBootstrap()
      .then((data) => {
        if (cancelled) return;
        setSettings(data.settings);
        setSystemLocale(data.systemLocale);
        setRunningKeys(new Set(data.runningKeys));
        setQuestTicks(Object.fromEntries(data.questTicks.map((tick) => [tick.key, tick])));
        setLibrary(data.library);
        setVersion(data.version);
        setPortable(data.portable);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setToast({ id: Date.now(), text: formatError(error), duration: 10_000 });
          setBootstrapError(formatError(error));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setReady(true);
          if (isTauri()) void getCurrentWindow().show();
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    document.documentElement.lang = language;
    if (i18n.language !== language) {
      void i18n.changeLanguage(language);
    }
  }, [language]);

  useEffect(() => {
    if (!isTauri() || !ready || portable) return;

    let cancelled = false;
    const timer = window.setTimeout(() => {
      void checkForUpdate()
        .then((update) => {
          if (cancelled) {
            if (update) void update.close();
            return;
          }
          if (!update) return;
          setAvailableUpdate(update);
          // Queue the prompt until all other dialogs have closed.
        })
        .catch(() => {
          // Update checks are best-effort and should never block app start.
        });
    }, 1200);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [ready, portable]);

  useEffect(() => {
    if (availableUpdate && !settingsOpen && !addGameOpen && !steamGameKey) setUpdateOpen(true);
  }, [availableUpdate, settingsOpen, addGameOpen, steamGameKey]);

  useEffect(() => {
    if (!isTauri()) return;
    return onDummyStarted(({ key }) => setRunningKeys((current) => new Set(current).add(key)));
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    return onDummyError((error) =>
      setToast({ id: Date.now(), text: formatError(error), duration: 10_000 }),
    );
  }, []);

  useEffect(() => {
    runningRef.current = runningKeys;
  }, [runningKeys]);

  useEffect(() => {
    if (!isTauri() || !ready || settingsOpen || addGameOpen || steamGameKey || installingUpdate)
      return;
    let cancelled = false;
    const reconcile = () => {
      void getBootstrap()
        .then((data) => {
          if (cancelled) return;
          setRunningKeys(new Set(data.runningKeys));
          setLibrary(data.library);
          setQuestTicks(Object.fromEntries(data.questTicks.map((tick) => [tick.key, tick])));
        })
        .catch(() => {});
    };
    window.addEventListener("focus", reconcile);
    return () => {
      cancelled = true;
      window.removeEventListener("focus", reconcile);
    };
  }, [ready, settingsOpen, addGameOpen, steamGameKey, installingUpdate]);

  useEffect(() => {
    const handleKey = (event: KeyboardEvent) => {
      if (settingsOpen || addGameOpen || steamGameKey || updateOpen) return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setSelectedKey(null);
        requestAnimationFrame(() => searchRef.current?.focus());
      }
      if (event.key === "Escape" && selectedKey && !(event.target instanceof HTMLInputElement)) {
        setSelectedKey(null);
        requestAnimationFrame(() => openedFrom.current?.focus());
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [settingsOpen, addGameOpen, steamGameKey, updateOpen, selectedKey]);

  useEffect(() => {
    if (!isTauri()) return;

    return onDummyExited((payload) => {
      setRunningKeys((current) => {
        const next = new Set(current);
        next.delete(payload.key);
        return next;
      });
      setQuestTicks((current) => {
        const next = { ...current };
        delete next[payload.key];
        return next;
      });
      delete pauseIntentRef.current[payload.key];
      setBusyKey((current) => (current === payload.key ? null : current));
    });
  }, []);

  useEffect(() => {
    if (!isTauri()) return;

    return onQuestTick((tick) => {
      if (!runningRef.current.has(tick.key)) return;
      const intent = pauseIntentRef.current[tick.key];
      if (intent && tick.paused !== intent.desired && Date.now() - intent.at < 2000) {
        return;
      }
      if (intent && tick.paused === intent.desired) {
        delete pauseIntentRef.current[tick.key];
      }

      setQuestTicks((current) => {
        const existing = current[tick.key];
        if (existing) {
          if (tick.timerId < existing.timerId) return current;
          if (
            tick.timerId === existing.timerId &&
            tick.remainingSeconds > existing.remainingSeconds
          ) {
            return current;
          }
        }
        return { ...current, [tick.key]: tick };
      });
    });
  }, []);

  useEffect(() => {
    if (selectedKey && !library.some((game) => makeGameKey(game) === selectedKey)) {
      setSelectedKey(null);
    }
  }, [library, selectedKey]);

  const games = useMemo(() => {
    const term = query.trim().toLowerCase();

    return library
      .map((game) => {
        const key = makeGameKey(game);
        const tick = questTicks[key];
        return {
          ...game,
          running: runningKeys.has(key),
          remaining: tick ? formatRemaining(tick.remainingSeconds) : undefined,
          phase: tick?.phase,
          percent: tick?.percent,
          paused: tick?.paused ?? false,
        };
      })
      .filter((game) => {
        if (filter === "favorites" && !game.isFavorite) return false;
        if (filter === "active" && !game.running) return false;
        if (term && !game.name.toLowerCase().includes(term)) return false;
        return true;
      })
      .sort(
        (left, right) =>
          Number(right.isFavorite) - Number(left.isFavorite) || left.name.localeCompare(right.name),
      );
  }, [filter, library, query, questTicks, runningKeys]);

  const steamGame = useMemo(() => {
    if (!steamGameKey) return null;
    const game = library.find((entry) => makeGameKey(entry) === steamGameKey);
    return game ? { ...game, running: runningKeys.has(steamGameKey) } : null;
  }, [library, runningKeys, steamGameKey]);

  const selectedGame = useMemo(() => {
    if (!selectedKey) return null;
    const game = library.find((entry) => makeGameKey(entry) === selectedKey);
    if (!game) return null;
    const tick = questTicks[selectedKey];
    return {
      ...game,
      running: runningKeys.has(selectedKey),
      remaining: tick ? formatRemaining(tick.remainingSeconds) : undefined,
      phase: tick?.phase,
      percent: tick?.percent,
      paused: tick?.paused ?? false,
    };
  }, [library, questTicks, runningKeys, selectedKey]);

  const showToast = (text: string) => {
    setToast({ id: Date.now(), text });
  };

  const handleCheckForUpdates = async (): Promise<"available" | "current"> => {
    if (portable) throw new Error(t("updates.portable"));
    const update = await checkForUpdate();
    if (!update) return "current";

    setAvailableUpdate(update);
    setUpdateError(null);
    setUpdateProgress(undefined);
    setInstallingUpdate(false);
    return "available";
  };

  const handleInstallUpdate = async () => {
    if (!availableUpdate || installingUpdate) return;
    if (runningKeys.size > 0) {
      setUpdateError(t("updates.stopFirst"));
      return;
    }

    setInstallingUpdate(true);
    setUpdateError(null);
    setUpdateProgress(0);

    try {
      await installUpdate(availableUpdate, (progress) => {
        setUpdateProgress(progress.percent);
      });
    } catch (error: unknown) {
      setUpdateError(formatError(error));
      setInstallingUpdate(false);
    }
  };

  const handleLaterUpdate = () => {
    if (installingUpdate) return;
    setUpdateOpen(false);
    setAvailableUpdate(null);
    setUpdateError(null);
    setUpdateProgress(undefined);
  };

  const openGame = (game: Game) => {
    openedFrom.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setSelectedKey(makeGameKey(game));
  };

  const closeGame = () => {
    setSelectedKey(null);
    requestAnimationFrame(() => openedFrom.current?.focus());
  };

  const handleAddGame = async (game: DatabaseGame) => {
    const added = await addGame(game);
    setLibrary((current) => {
      if (current.some((entry) => makeGameKey(entry) === makeGameKey(added))) {
        return current;
      }
      return [...current, added];
    });
    setSelectedKey(makeGameKey(added));
  };

  const handleToggleFavorite = async (game: Game) => {
    try {
      const updated = await toggleFavorite(game.appId, game.exe);
      setLibrary((current) =>
        current.map((entry) => (makeGameKey(entry) === makeGameKey(updated) ? updated : entry)),
      );
    } catch (error: unknown) {
      showToast(formatError(error));
    }
  };

  const handleToggleRunning = async (game: Game) => {
    const gameKey = makeGameKey(game);
    if (busyKey === gameKey) return;

    const wasRunning = runningKeys.has(gameKey);
    setBusyKey(gameKey);
    delete pauseIntentRef.current[gameKey];

    try {
      if (wasRunning) {
        await stopDummy(gameKey);
        setRunningKeys((current) => {
          const next = new Set(current);
          next.delete(gameKey);
          return next;
        });
        setQuestTicks((current) => {
          const next = { ...current };
          delete next[gameKey];
          return next;
        });
      } else {
        const result = await launchDummy({
          appId: game.appId,
          exe: game.exe,
        });
        setRunningKeys((current) => new Set(current).add(result.gameKey));
      }
    } catch (error: unknown) {
      const message = formatError(error);
      const prefix = wasRunning ? t("notices.stopFailed") : t("notices.launchFailed");
      showToast(`${prefix} ${message}`);
    } finally {
      setBusyKey(null);
    }
  };

  const handleTogglePause = async (game: Game) => {
    const gameKey = makeGameKey(game);
    if (!runningKeys.has(gameKey) || !settings.questTimerEnabled) return;

    const desired = !game.paused;
    pauseIntentRef.current[gameKey] = { desired, at: Date.now() };

    try {
      if (desired) {
        await pauseDummy(gameKey);
      } else {
        await resumeDummy(gameKey);
      }

      setQuestTicks((current) => {
        const tick = current[gameKey];
        if (!tick) return current;
        return {
          ...current,
          [gameKey]: { ...tick, paused: desired },
        };
      });
    } catch (error: unknown) {
      delete pauseIntentRef.current[gameKey];
      showToast(formatError(error));
    }
  };

  const handleUpdateExe = async (game: Game, exe: string) => {
    try {
      const updated = await updateGameExecutable(game.appId, game.exe, exe);
      setLibrary((current) =>
        current.map((entry) => (makeGameKey(entry) === makeGameKey(game) ? updated : entry)),
      );
      setSelectedKey(makeGameKey(updated));
    } catch (error: unknown) {
      showToast(formatError(error));
      throw error;
    }
  };

  const handleSaveSteam = async (request: SteamSetupRequest) => {
    try {
      const result = await setupSteamIntegration(request);
      const oldKey = `${request.appId}::${request.currentExe}`;
      setLibrary((current) =>
        current.map((entry) => (makeGameKey(entry) === oldKey ? result.updatedGame : entry)),
      );
      setSelectedKey((current) => (current === oldKey ? makeGameKey(result.updatedGame) : current));
      setSteamGameKey(null);
      showToast(t("notices.steamSaved"));
    } catch (error: unknown) {
      showToast(formatError(error));
      throw error;
    }
  };

  const handleRemoveSteam = async () => {
    if (!steamGame) return;
    try {
      const result = await removeSteamIntegration(steamGame.appId, steamGame.exe);
      if (result.updatedGame) {
        const updated = result.updatedGame;
        setLibrary((current) =>
          current.map((entry) => (makeGameKey(entry) === makeGameKey(steamGame) ? updated : entry)),
        );
        setSelectedKey((current) =>
          current === makeGameKey(steamGame) ? makeGameKey(updated) : current,
        );
      }
      setSteamGameKey(null);
      showToast(t("notices.steamRemoved"));
    } catch (error: unknown) {
      showToast(formatError(error));
      throw error;
    }
  };

  const handleCreateShortcut = async (game: Game) => {
    try {
      const path = await createShortcut(game.appId, game.exe);
      showToast(t("notices.shortcutCreated", { path }));
    } catch (error: unknown) {
      showToast(formatError(error));
    }
  };

  const handleDeleteGame = async (game: Game) => {
    try {
      await removeGame(game.appId, game.exe);
      setLibrary((current) => current.filter((entry) => makeGameKey(entry) !== makeGameKey(game)));
      closeGame();
    } catch (error: unknown) {
      showToast(formatError(error));
      throw error;
    }
  };

  const handleOpenSettings = () => {
    setSettingsSnapshot(settings);
    setSettingsOpen(true);
  };

  const handlePreviewSettings = (next: Settings) => {
    setSettings(next);
  };

  const handleCancelSettings = () => {
    setSettings(settingsSnapshot);
    setSettingsOpen(false);
  };

  const handleSaveSettings = async (next: Settings) => {
    try {
      const saved = await saveSettings(next);
      setSettings(saved);
      setSettingsOpen(false);
      showToast(t("notices.settingsSaved"));
    } catch (error: unknown) {
      showToast(formatError(error));
    }
  };

  return (
    <FluentProvider
      theme={resolvedTheme === "dark" ? darkFluentTheme : lightFluentTheme}
      style={{ height: "100%", background: "transparent" }}
    >
      <div className="app-shell">
        <TitleBar />

        <main className="library">
          <div className="library-view" inert={selectedGame !== null}>
            <header className="command-bar">
              <label className="search-box">
                <Search20Regular aria-hidden="true" />
                <input
                  ref={searchRef}
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={t("library.searchPlaceholder")}
                  aria-label={t("library.searchPlaceholder")}
                  spellCheck={false}
                />
                <kbd className="keycap">{t("search.shortcut")}</kbd>
              </label>

              <fieldset className="filter-pills" aria-label={t("library.title")}>
                {filters.map((item) => (
                  <button
                    key={item}
                    type="button"
                    aria-pressed={filter === item}
                    className={`filter-pill${filter === item ? " is-active" : ""}`}
                    onClick={() => setFilter(item)}
                  >
                    {t(`library.${item}`)}
                  </button>
                ))}
              </fieldset>

              <div className="command-bar__actions">
                <button
                  type="button"
                  className="icon-button"
                  aria-label={t("settings.title")}
                  title={t("settings.title")}
                  onClick={handleOpenSettings}
                  disabled={!ready || !!bootstrapError}
                >
                  <Settings20Regular />
                </button>
                <button
                  type="button"
                  className="primary-button"
                  onClick={() => setAddGameOpen(true)}
                  disabled={!ready || !!bootstrapError}
                >
                  <Add16Filled aria-hidden="true" />
                  {t("library.addGame")}
                </button>
              </div>
            </header>

            <div className="library__heading">
              <h1>{t("library.title")}</h1>
              <span className="library__count">{t("library.count", { count: games.length })}</span>

              <fieldset className="view-toggle" aria-label={t("library.viewMode")}>
                <button
                  type="button"
                  className={`icon-button icon-button--small${view === "grid" ? " is-active" : ""}`}
                  onClick={() => setView("grid")}
                  aria-pressed={view === "grid"}
                  title={t("library.grid")}
                  aria-label={t("library.grid")}
                >
                  <Grid20Regular />
                </button>
                <button
                  type="button"
                  className={`icon-button icon-button--small${view === "list" ? " is-active" : ""}`}
                  onClick={() => setView("list")}
                  aria-pressed={view === "list"}
                  title={t("library.list")}
                  aria-label={t("library.list")}
                >
                  <List20Regular />
                </button>
              </fieldset>
            </div>

            {bootstrapError ? (
              <section className="empty-state" role="alert">
                <p>{bootstrapError}</p>
                <button
                  type="button"
                  className="secondary-button"
                  onClick={() => location.reload()}
                >
                  {t("common.retry")}
                </button>
              </section>
            ) : !ready ? (
              <p role="status">{t("catalog.loading")}</p>
            ) : games.length === 0 ? (
              <section className="empty-state">
                <h2>{t(library.length === 0 ? "library.emptyTitle" : "library.noMatches")}</h2>
                <p>{t(library.length === 0 ? "library.emptyBody" : "library.noMatchesBody")}</p>
                <button
                  type="button"
                  className="primary-button"
                  onClick={() => {
                    if (library.length === 0) setAddGameOpen(true);
                    else {
                      setQuery("");
                      setFilter("all");
                    }
                  }}
                >
                  {t(library.length === 0 ? "library.emptyAction" : "library.clearFilters")}
                </button>
              </section>
            ) : view === "grid" ? (
              <PosterGrid
                games={games}
                cardsPerRow={settings.cardsPerRow}
                maxCardWidth={settings.maxCardWidth}
                canPause={settings.questTimerEnabled}
                onOpenGame={openGame}
                onToggleFavorite={handleToggleFavorite}
                onToggleRunning={handleToggleRunning}
                onTogglePause={handleTogglePause}
                busyKey={busyKey}
              />
            ) : (
              <GameList
                games={games}
                busyKey={busyKey}
                onOpen={openGame}
                onToggleRunning={handleToggleRunning}
                onToggleFavorite={handleToggleFavorite}
              />
            )}
          </div>

          {selectedGame ? (
            <div className="detail-view">
              <GameDetail
                key={selectedKey}
                game={selectedGame}
                busy={busyKey === makeGameKey(selectedGame)}
                canPause={settings.questTimerEnabled}
                onBack={closeGame}
                onConfigureSteam={(game) => setSteamGameKey(makeGameKey(game))}
                onCreateShortcut={handleCreateShortcut}
                onDelete={handleDeleteGame}
                onToggleFavorite={handleToggleFavorite}
                onToggleRunning={handleToggleRunning}
                onTogglePause={handleTogglePause}
                onUpdateExe={handleUpdateExe}
              />
            </div>
          ) : null}
        </main>

        <AddGameDialog open={addGameOpen} onOpenChange={setAddGameOpen} onAdd={handleAddGame} />

        <SteamDialog
          open={steamGame !== null}
          game={steamGame}
          onOpenChange={(open) => {
            if (!open) setSteamGameKey(null);
          }}
          onRemove={handleRemoveSteam}
          onSave={handleSaveSteam}
        />

        <SettingsDialog
          open={settingsOpen}
          settings={settings}
          version={version}
          portable={portable}
          onPreview={handlePreviewSettings}
          onCancel={handleCancelSettings}
          onDismiss={handleCancelSettings}
          onSave={handleSaveSettings}
          onCheckForUpdates={handleCheckForUpdates}
        />

        {availableUpdate && updateOpen ? (
          <UpdateDialog
            update={availableUpdate}
            installing={installingUpdate}
            progress={updateProgress}
            error={updateError}
            onInstall={() => void handleInstallUpdate()}
            onLater={handleLaterUpdate}
          />
        ) : null}

        {toast ? (
          <Toast
            key={toast.id}
            message={toast.text}
            actionLabel={toast.actionLabel}
            duration={toast.duration}
            onAction={toast.onAction}
            onDismiss={() => setToast(null)}
          />
        ) : null}
      </div>
    </FluentProvider>
  );
}
