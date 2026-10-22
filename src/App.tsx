import { FluentProvider, webDarkTheme, webLightTheme } from "@fluentui/react-components";
import {
  Add16Filled,
  Grid20Regular,
  List20Regular,
  Search20Regular,
  Settings20Regular,
} from "@fluentui/react-icons";
import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import AddGameDialog from "./components/AddGameDialog";
import GameDetail from "./components/GameDetail";
import GameList from "./components/GameList";
import PosterGrid from "./components/PosterGrid";
import SettingsDialog from "./components/SettingsDialog";
import SteamDialog from "./components/SteamDialog";
import TitleBar from "./components/TitleBar";
import Toast from "./components/Toast";
import i18n from "./i18n";
import {
  addGame,
  createShortcut,
  getBootstrap,
  launchDummy,
  onDummyExited,
  onQuestTick,
  removeGame,
  removeSteamIntegration,
  saveSettings,
  setupSteamIntegration,
  stopDummy,
  toggleFavorite,
  updateGameExecutable,
} from "./lib/backend";
import { useResolvedTheme } from "./theme/useResolvedTheme";
import {
  type DatabaseGame,
  DEFAULT_SETTINGS,
  type Game,
  type GameFilter,
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
  const [library, setLibrary] = useState<Game[]>([]);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [steamGameKey, setSteamGameKey] = useState<string | null>(null);
  const [addGameOpen, setAddGameOpen] = useState(false);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [settingsSnapshot, setSettingsSnapshot] = useState<Settings>(DEFAULT_SETTINGS);
  const [version, setVersion] = useState("1.0.0");
  const [systemLocale, setSystemLocale] = useState("en");
  const [runningKeys, setRunningKeys] = useState<Set<string>>(() => new Set());
  const [questTicks, setQuestTicks] = useState<Record<string, QuestTick>>({});
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [toast, setToast] = useState<{ id: number; text: string } | null>(null);

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
        setLibrary(data.library);
        setVersion(data.version);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setToast({ id: Date.now(), text: String(error) });
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (i18n.language !== language) {
      void i18n.changeLanguage(language);
    }
  }, [language]);

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
      setBusyKey((current) => (current === payload.key ? null : current));
    });
  }, []);

  useEffect(() => {
    if (!isTauri()) return;

    return onQuestTick((tick) => {
      setQuestTicks((current) => ({ ...current, [tick.key]: tick }));
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
        };
      })
      .filter((game) => {
        if (filter === "favorites" && !game.isFavorite) return false;
        if (filter === "active" && !game.running) return false;
        if (term && !game.name.toLowerCase().includes(term)) return false;
        return true;
      });
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
    };
  }, [library, questTicks, runningKeys, selectedKey]);

  const showToast = (text: string) => {
    setToast({ id: Date.now(), text });
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
      showToast(error instanceof Error ? error.message : String(error));
    }
  };

  const handleToggleRunning = async (game: Game) => {
    const gameKey = makeGameKey(game);
    if (busyKey === gameKey) return;

    const wasRunning = runningKeys.has(gameKey);
    setBusyKey(gameKey);

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
          name: game.name,
          exe: game.exe,
        });
        setRunningKeys((current) => new Set(current).add(result.gameKey));
      }
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      const prefix = wasRunning ? t("notices.stopFailed") : t("notices.launchFailed");
      showToast(`${prefix} ${message}`);
    } finally {
      setBusyKey(null);
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
      showToast(error instanceof Error ? error.message : String(error));
      throw error;
    }
  };

  const handleSaveSteam = async (request: SteamSetupRequest) => {
    try {
      const result = await setupSteamIntegration(request);
      setLibrary((current) =>
        current.map((entry) =>
          makeGameKey(entry) === makeGameKey(result.updatedGame) ? result.updatedGame : entry,
        ),
      );
      setSteamGameKey(null);
      showToast(t("notices.steamSaved"));
    } catch (error: unknown) {
      showToast(error instanceof Error ? error.message : String(error));
      throw error;
    }
  };

  const handleRemoveSteam = async () => {
    if (!steamGame) return;
    try {
      const result = await removeSteamIntegration(
        steamGame.appId,
        steamGame.steamAppId,
        steamGame.installDir,
      );
      if (result.updatedGame) {
        setLibrary((current) =>
          current.map((entry) =>
            makeGameKey(entry) === makeGameKey(result.updatedGame as Game)
              ? (result.updatedGame as Game)
              : entry,
          ),
        );
      }
      setSteamGameKey(null);
      showToast(t("notices.steamRemoved"));
    } catch (error: unknown) {
      showToast(error instanceof Error ? error.message : String(error));
      throw error;
    }
  };

  const handleCreateShortcut = async (game: Game) => {
    try {
      const path = await createShortcut(game.appId, game.exe);
      showToast(t("notices.shortcutCreated", { path }));
    } catch (error: unknown) {
      showToast(error instanceof Error ? error.message : String(error));
    }
  };

  const handleDeleteGame = async (game: Game) => {
    try {
      await removeGame(game.appId, game.exe);
      setLibrary((current) => current.filter((entry) => makeGameKey(entry) !== makeGameKey(game)));
      setSelectedKey(null);
    } catch (error: unknown) {
      showToast(error instanceof Error ? error.message : String(error));
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

  const handleDismissSettings = async () => {
    try {
      const saved = await saveSettings(settings);
      setSettings(saved);
      setSettingsOpen(false);
      showToast(t("notices.settingsSaved"));
    } catch (error: unknown) {
      setSettings(settingsSnapshot);
      setSettingsOpen(false);
      showToast(error instanceof Error ? error.message : String(error));
    }
  };

  const handleSaveSettings = async (next: Settings) => {
    try {
      const saved = await saveSettings(next);
      setSettings(saved);
      setSettingsOpen(false);
      showToast(t("notices.settingsSaved"));
    } catch (error: unknown) {
      showToast(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <FluentProvider
      theme={resolvedTheme === "dark" ? darkFluentTheme : lightFluentTheme}
      style={{ height: "100%", background: "transparent" }}
    >
      <div className="app-shell">
        <TitleBar />

        {selectedGame ? (
          <main className="library library--detail">
            <GameDetail
              key={selectedKey}
              game={selectedGame}
              busy={busyKey === makeGameKey(selectedGame)}
              onBack={() => setSelectedKey(null)}
              onConfigureSteam={(game) => setSteamGameKey(makeGameKey(game))}
              onCreateShortcut={handleCreateShortcut}
              onDelete={handleDeleteGame}
              onToggleFavorite={handleToggleFavorite}
              onToggleRunning={handleToggleRunning}
              onUpdateExe={handleUpdateExe}
            />
          </main>
        ) : (
          <main className="library">
            <header className="command-bar">
              <label className="search-box">
                <Search20Regular aria-hidden="true" />
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={t("library.searchPlaceholder")}
                  aria-label={t("library.searchPlaceholder")}
                  spellCheck={false}
                />
                <kbd className="keycap">{t("search.shortcut")}</kbd>
              </label>

              <div className="filter-pills" role="tablist" aria-label={t("library.title")}>
                {filters.map((item) => (
                  <button
                    key={item}
                    type="button"
                    role="tab"
                    aria-selected={filter === item}
                    className={`filter-pill${filter === item ? " is-active" : ""}`}
                    onClick={() => setFilter(item)}
                  >
                    {t(`library.${item}`)}
                  </button>
                ))}
              </div>

              <div className="command-bar__actions">
                <button
                  type="button"
                  className="icon-button"
                  aria-label={t("settings.title")}
                  title={t("settings.title")}
                  onClick={handleOpenSettings}
                >
                  <Settings20Regular />
                </button>
                <button
                  type="button"
                  className="primary-button"
                  onClick={() => setAddGameOpen(true)}
                >
                  <Add16Filled aria-hidden="true" />
                  {t("library.addGame")}
                </button>
              </div>
            </header>

            <div className="library__heading">
              <h1>{t("library.title")}</h1>
              <span className="library__count">{t("library.count", { count: games.length })}</span>

              <fieldset className="view-toggle" aria-label="View mode">
                <button
                  type="button"
                  className={`icon-button icon-button--small${view === "grid" ? " is-active" : ""}`}
                  onClick={() => setView("grid")}
                  aria-pressed={view === "grid"}
                  title="Grid"
                >
                  <Grid20Regular />
                </button>
                <button
                  type="button"
                  className={`icon-button icon-button--small${view === "list" ? " is-active" : ""}`}
                  onClick={() => setView("list")}
                  aria-pressed={view === "list"}
                  title="List"
                >
                  <List20Regular />
                </button>
              </fieldset>
            </div>

            {view === "grid" ? (
              <PosterGrid
                games={games}
                cardsPerRow={settings.cardsPerRow}
                onAddGame={() => setAddGameOpen(true)}
                onOpenGame={(game) => setSelectedKey(makeGameKey(game))}
                onToggleFavorite={handleToggleFavorite}
                onToggleRunning={handleToggleRunning}
                busyKey={busyKey}
              />
            ) : (
              <GameList games={games} />
            )}
          </main>
        )}

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
          onPreview={handlePreviewSettings}
          onCancel={handleCancelSettings}
          onDismiss={handleDismissSettings}
          onSave={handleSaveSettings}
        />

        {toast ? (
          <Toast key={toast.id} message={toast.text} onDismiss={() => setToast(null)} />
        ) : null}
      </div>
    </FluentProvider>
  );
}
