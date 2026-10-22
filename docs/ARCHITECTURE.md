---
status: draft
name: Dummies Architecture
lastUpdated: "2026-10-21"
---

# Dummies — Architecture and Migration Plan

## Decision

Build the next version of the launcher as a **Tauri 2** desktop application:

- **Shell and backend:** Rust.
- **UI:** React + TypeScript + Vite.
- **Components and behavior:** Fluent UI React v9, visually overridden by the
  `docs/DESIGN.md` tokens.
- **Dummy process:** a new Rust/Win32 executable with no .NET, no WebView, and no
  sidecar files.
- **Persistence:** JSON in the app data directory, with atomic writes and schema
  versioning.
- **Updates:** Tauri updater with signed artifacts.
- **Installer:** NSIS per-user installer plus a standalone portable executable.
- **Localization:** English and Spanish, system default, English fallback.
- **License:** MIT, with upstream attribution in `THIRD_PARTY_NOTICES.md`.

This is a controlled rewrite, not an in-place Electron migration. The Electron and C#
implementations stay in `legacy/` (or on a branch) until feature parity is verified.

## Current status (2026-10-22)

- Tauri 2 shell scaffolded in this folder.
- Raycast-inspired library surface implemented with React/Fluent UI primitives.
- System light/dark detection and English/Spanish locale setup implemented.
- Rust/Win32 dummy implemented in `crates/dummy-game`.
- `dummy-game.exe` is 779 KiB; the transparent layered window uses ~3.0 MB private working set.
- Release shell measured ~147 MB private working set (Task Manager-style) with a real
  28-entry library and remote covers; the Rust app process itself is ~14.3 MB.
- Settings persistence, system locale detection, dummy launch/stop, running-state
  events and the settings dialog are implemented.
- Settings preview live; Cancel reverts, backdrop/Escape saves, saved feedback uses a
  fixed overlay toast, and cards-per-row (4-8) is configurable.
- Discord catalog sync/search/add, persistent library, legacy migration, game detail,
  favorite, executable override, delete, shortcuts, Steam manifests, tray controls,
  quest timers, notifications and manual GitHub update check are implemented.
- NSIS installer is 3.79 MiB; portable ZIP is 5.02 MiB; release app executable is
  14.22 MiB.
- Signed auto-updater is implemented for 1.0.2 and later; Windows code signing is still
  pending. Portable packaging is implemented.

## Goals

1. Reduce idle memory versus the current Electron build, without pretending WebView2
   is free.
2. Reduce the per-game dummy cost dramatically: one small Rust executable, one copy
   per game, no .NET runtime per process.
3. Move process control, quest timers, auto-stop, and notifications out of the
   renderer into Rust.
4. Reproduce the current feature set: Discord detectable catalog, library, favorites,
   executable override, multiple running games, tray, Steam manifest integration,
   shortcuts, updates, and quest automation.
5. Deliver a Raycast-inspired visual system with a native Windows title bar, light and
   dark modes, optional Mica/Acrylic, and English/Spanish localization.
6. Keep the application usable without administrator rights for normal operation.

## Non-goals for 1.0

- Mobile, macOS, or Linux support.
- A full game launcher or Rich Presence client.
- A plugin system.
- Cloud sync or user accounts.
- A custom native renderer. Tauri uses WebView2 by design.

## Trade-offs

### Tauri vs pure native Rust

Tauri keeps the visual and iteration advantages of a web UI while removing the
bundled Chromium/Node runtime. WebView2 still consumes memory, so the realistic target
is roughly 90–130 MB private working set in the app process tree, not 20 MB. The
release scaffold measured ~113 MB private working set on this machine; the Rust process
itself was ~3.6 MB. The spike in Phase 0 must keep measuring this on the target machine
before the full rewrite.

If the measured memory is unacceptable, the fallback options are:

1. Destroy the WebView window when minimized to tray, keeping only the Rust core and
   tray. Recreate the window when the user opens it.
2. Move to a native Rust UI (Slint or iced) with a custom design system. This is a
   larger effort and loses Fluent UI component coverage.

### Electron vs Tauri update path

The Electron app is a separate product identity. The Rust app will use a new
identifier (`com.morphy137.dummies`), a new installer name (`Dummies`), and a new data
directory. This avoids installer and updater collisions. On first run, the Rust app
imports the old Electron data into the new location and leaves the old files intact.

Old Electron users must install the new app once. The old app cannot auto-update into
a Tauri build. A final Electron notice release or a release-notes migration guide is a
release-engineering decision, not a code dependency.

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│ React UI (Fluent UI v9 + Dummies tokens)                   │
│  library / detail / add / settings / activity / dialogs     │
└──────────────────────────┬──────────────────────────────────┘
                           │ typed commands + events (tauri-specta)
┌──────────────────────────▼──────────────────────────────────┐
│ Tauri shell (Rust)                                          │
│  window, tray, menu, updater, notifications, single instance│
├─────────────────────────────────────────────────────────────┤
│ app-core                                                    │
│  models, paths, settings, database, library, covers,       │
│  quest timers, versioning, validation, migration           │
├─────────────────────────────────────────────────────────────┤
│ services                                                    │
│  discord API, process manager, steam, shortcuts, logging    │
├─────────────────────────────────────────────────────────────┤
│ platform-win                                                │
│  registry, DWM/Mica, COM shortcuts, job objects, locale     │
└──────────────────────────┬──────────────────────────────────┘
                           │ spawn + stdin
┌──────────────────────────▼──────────────────────────────────┐
│ dummy-game.exe (Rust/Win32)                                 │
│  transparent layered window, embedded Mood frames, no WebView/sidecars        │
└─────────────────────────────────────────────────────────────┘
```

## Workspace layout

```text
dummies-launcher/             # isolated migration project; old Electron app stays untouched
  Cargo.toml                  # workspace
  crates/
    app-core/                 # pure Rust domain and services
    dummy-game/               # Win32 dummy process
  src-tauri/                  # Tauri application crate
    Cargo.toml
    tauri.conf.json
    capabilities/default.json
    icons/
    src/
      main.rs
      state.rs
      commands/
      services/
      platform/
  src/                        # React frontend
    App.tsx
    main.tsx
    i18n/
    theme/
    features/
      library/
      game/
      database/
      settings/
      activity/
    components/
  docs/
    DESIGN.md
    ARCHITECTURE.md
    qa-checklist.md
    migration.md
```

Use a Cargo workspace so `app-core` and `dummy-game` can be tested without Tauri.

## Domain model

Persist the same field names as the current app where possible, using camelCase for
compatibility with existing JSON:

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryGame {
    app_id: String,
    name: String,
    exe: String,
    is_favorite: bool,
    steam_app_id: Option<String>,
    icon_hash: Option<String>,
    cover_image_hash: Option<String>,
    requires_steam: bool,
    use_steam_path: bool,
    install_dir: Option<String>,
    steam_exe_path: Option<PathBuf>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}
```

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    schema_version: u32,
    quest_timer_enabled: bool,
    quest_duration_minutes: u32,
    quest_sync_buffer_seconds: u32,
    auto_stop_on_complete: bool,
    notify_on_complete: bool,
    dummy_hide_on_minimize: bool,
    launcher_close_to_tray: bool,
    preferred_view_mode: ViewMode,
    card_size: CardSize,
    theme_mode: ThemeMode,       // system | dark | light
    material_mode: MaterialMode, // solid | mica | acrylic
    accent_mode: AccentMode,     // system | custom
    accent_color: Option<String>,
    reduce_motion: bool,
    language: LanguageMode,      // system | en | es
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}
```

Keep `#[serde(default)]` and `#[serde(flatten)]` so old settings and future fields do
not break loading.

## Data locations

Use the Tauri app data directory based on `com.morphy137.dummies`:

- `%APPDATA%\com.morphy137.dummies\myGames.json` (implemented)
- `%APPDATA%\com.morphy137.dummies\settings.json` (implemented)
- `%APPDATA%\com.morphy137.dummies\gamelist.json` (implemented)
- `%APPDATA%\com.morphy137.dummies\games\<appId>\<exe path>` (implemented)
- `%LOCALAPPDATA%\com.morphy137.dummies\logs\`
- WebView2 Cache Storage for downloaded cover images (implemented in
  `src/lib/coverCache.ts`); failed candidates are remembered in memory for the session.

Writes are atomic: write to a temporary file in the same directory, flush, then rename.
Keep a `.bak` of the previous file when overwriting.

## Migration from Electron

The old Electron app may store data under one of:

- `%APPDATA%\discord-fake-game-launcher-ui`
- `%APPDATA%\Discord Fake Game Launcher`
- `%APPDATA%\discord-fake-game-launcher`
- A renamed or versioned variant found by scanning for `myGames.json`

Migration rules:

1. Never delete or modify the old directory.
2. Copy `myGames.json`, `settings.json`, and `gamelist.json` into the new directory.
3. Create a timestamped backup of the copied files.
4. Map `minimizeToTray` to `dummyHideOnMinimize`; add `launcherCloseToTray = true`.
5. Leave the old `games/` folder in place. Regenerate dummies with the Rust binary on
   demand. Offer a cleanup action in Settings later.
6. If a file is corrupt, log the error and continue with defaults rather than blocking
   startup.

Migration must be idempotent and covered by fixtures for each known shape.

## Dummy process design

This is the main resource optimization.

### Requirements

- The process name must be the renamed executable file name; Discord detects it.
- No .NET runtime, no WebView2, no Node, no sidecar DLLs.
- Visible by default with a transparent 240x270 layered window.
- Resizable from any edge or corner, with a 180x210 minimum and a 640x720 maximum.
- Random pre-rendered Mood art on every launch, with a blink animation. The art scales
  with the window and the controls stay centered.
- Minimal custom frame with minimize and close only (no maximize).
- A normal 64x64 detection window sits under the face so Discord still sees a
  conventional titled top-level window.
- stdin commands: `SHOW`, `HIDE`, `QUIT`, `STOP`.
- Low disk usage and low per-process memory.

### Implementation

- Crate: `crates/dummy-game`.
- Windows bindings: `windows-sys` (raw Win32, smaller than the full `windows` crate).
- `#![windows_subsystem = "windows"]` to avoid a console window.
- Release profile:
  ```toml
  [profile.release]
  opt-level = "z"
  lto = "fat"
  codegen-units = 1
  panic = "abort"
  strip = true
  ```
- Link the CRT statically (`+crt-static`) so the dummy has no VC runtime DLL
  dependency.
- The visual window is a `WS_EX_LAYERED` top-level window drawn with
  `UpdateLayeredWindow` from a 32-bit premultiplied BGRA DIB.
- Each Mood has open-eye and closed-eye PNG frames embedded in the binary at 512x512.
  They are scaled to the current square art area when the window is resized. A 50 ms
  timer toggles the eye frame to reproduce the SVG blink animation.
- The source SVGs only define that blink. A native layered window has no SVG/CSS
  animation engine; two frames reproduce it without WebView2. Richer SVG animations
  would require a WebView2 Composition/DirectComposition host.
- The custom top strip is drawn into the same bitmap; it contains centered minimize and
  close hit areas and no maximize control. Dragging any edge or corner resizes the
  window within the 180x210–640x720 limits.
- A normal 64x64 detection window is placed directly under an opaque part of the face.
  It carries the game title and keeps Discord's window scan working despite the
  layered visual surface.
- A reader thread reads stdin line by line and posts `WM_APP_SHOW`, `WM_APP_HIDE`, or
  `WM_APP_QUIT` to the visual window.
- `SHOW` restores and focuses the layered window; `HIDE` hides it; `QUIT` exits.
- If stdin reaches EOF, the dummy exits. The launcher keeps the process alive while
  the app is running and stops it explicitly on quit.
- The launcher creates a Windows Job Object with
  `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and assigns each dummy. If the launcher crashes,
  the job closes and the dummies exit instead of becoming orphans.

### Targets

| Metric | Target | Measured |
|---|---|---:|
| Dummy executable size | < 1 MB | 779 KiB |
| Hidden dummy private working set | < 5 MB | ~3.0 MB |
| Visible dummy private working set | < 10 MB | ~3.0 MB |
| Idle CPU | ~0% | ~0% |
| Disk per game | ~1 MB | ~779 KiB |
| Startup | < 50 ms | ~instant |

The old self-contained C# WinForms dummy can use tens of MB per process and copies a
large template per game. Replacing it is more important than shaving another 10 MB
from the shell. The Rust dummy is now the reference implementation.

### Copy and versioning

- Bundle the Rust dummy as a Tauri resource (for example,
  `resources/dummy-game.exe`).
- Copy it to `games/<appId>/<exe path>` and rename it to the target executable name.
- Because the filename must match the game, do not put a version in the filename.
  Compare the destination file hash/size with the bundled template and replace it when
  it differs.
- A single file copy is enough; there are no sidecars.

## Process manager

- `tokio::process::Command` with piped stdin, null stdout/stderr, and the game working
  directory.
- Store `GameKey = "<appId>::<exe>"` in a `Mutex<HashMap<GameKey, ManagedProcess>>`.
- `ManagedProcess` owns the child handle, stdin, start time, and quest timer state.
- A monitor task calls `try_wait()` or awaits `wait()` and emits `game://exited` with
  the key, exit code, app ID, exe, and name.
- `stop_game` sends `QUIT\n`, waits briefly, then kills the process if needed.
- `stop_all` iterates the map and emits a single state update.
- On launcher quit, stop all dummies or close the job object. Do not leave orphans.
- The launcher stays alive in the tray when the window is closed, so quest timers and
  auto-stop continue to work while the UI is hidden.

## Quest timers

Move timers out of the renderer:

- `QuestTimer { start: Instant, target: Duration, buffer: Duration, game: GameKey }`.
- A single Tokio interval ticks once per second while at least one timer exists.
- Emit `quest://tick` with `{ key, remainingMs, phase, percent }`; the UI updates only
  the visible timer components.
- On `phase == complete`:
  - If auto-stop is enabled, notify, stop the game, and emit `game://exited`.
  - Else notify once and keep the process running.
- If the window is hidden, timers and notifications still run.
- If the app restarts, active timers are not resumed. The launcher should not restart
  while games are running; closing the window goes to the tray.

## Steam integration

Keep the current behavior and data shape, with hardening:

- Read `HKCU\Software\Valve\Steam\SteamPath` with `winreg`; fall back to the standard
  Program Files locations.
- Validate numeric `steamAppId`.
- Validate `installDir` and `exe` against path traversal. Reject `.` and `..`;
  canonicalize and ensure the target remains under `steamapps\common\<installDir>`.
- Write the ACF with the same fields as the current app; keep a fixture test so the
  format does not drift.
- Copy the Rust dummy into the Steam folder and update the library entry
  (`useSteamPath`, `steamAppId`, `installDir`, `steamExePath`).
- Remove the manifest and dummy folder only after an explicit confirmation. The current
  app deletes without confirmation; the new UI should warn.
- If the Steam folder is not writable, return an actionable error instead of silently
  failing. Do not auto-elevate.

## Shortcuts

Use the Windows Shell COM API (`IShellLinkW` + `IPersistFile`) or a maintained small
crate. Set:

- Target: the dummy executable.
- Arguments: the display name.
- Working directory: the dummy folder.
- Icon: the game/app icon if available.

Avoid overwriting existing shortcuts; append `(2)`, `(3)`, etc.

## Database sync and search

- `reqwest` with rustls, a 30-second timeout, and a project User-Agent.
- Download the Discord detectable JSON to a temporary file, compare with the current
  file, and atomically replace it. Keep a `.bak`.
- Parse with `serde_json` inside `spawn_blocking`; the catalog is large and must not
  block the UI.
- Normalize to `DatabaseGame` with lowercase search keys.
- Search and paginate in Rust; return 100–200 rows per page. The UI virtualizes the
  list.
- If the network fails, use the cached file. If there is no cache, show an actionable
  empty state and keep the existing library usable.
- Enrich library entries from the database on load, but never overwrite user overrides
  such as the executable name.

## Theme and window integration

- Tauri 2 `setEffects` supports Mica/Acrylic, but requires a transparent window and a
  transparent page background. Use it for the title bar/backdrop only; keep content
  surfaces opaque.
- Windows 11: Mica available but off by default (Solid preserves the Raycast canvas).
  Windows 10: Solid fallback; Acrylic optional and off by default.
- Respect `EnableTransparency` and high contrast; force Solid when disabled.
- Read light/dark from the system and listen for theme changes. Apply the theme before
  showing the window to avoid a flash.
- Read the system accent color and use it only for focus rings, selection, and progress.
  The primary CTA remains white (dark) or black (light).
- Custom title bar: 48px, drag region, double-click maximize, caption buttons, and snap
  layouts. Evaluate `tauri-plugin-window-integration` for accent and snap geometry; if
  it is not stable, fall back to native decorations.
- Remember window size and position with `tauri-plugin-window-state`; clamp to the
  visible monitor area.

## Localization

- `i18next` + `react-i18next`, or a typed dictionary with the same features. Use
  `i18next` for interpolation/plural support.
- Locales: `en` and `es`.
- Default: system language. Fallback: English.
- Detect the system language in Rust (`GetUserDefaultUILanguage`) or via the Tauri OS
  plugin; map Spanish variants to `es`, everything else to `en`.
- Settings offer System / English / Español.
- No user-facing string in components. A Vitest test compares key sets between locales.
- Logs may use technical English, but UI log entries use locale keys.

## Security

- No broad shell plugin. The backend spawns processes through typed commands.
- Validate all paths and reject traversal. Never trust a game `exe` or Steam
  `installDir` from JSON or the frontend.
- Restrict Tauri capabilities to the commands actually used.
- Strict CSP; allow remote images only from the Discord and Steam CDNs, and local
  covers only through a scoped asset protocol/custom protocol.
- Sanitize release notes with `react-markdown` + `rehype-sanitize`; no
  `dangerouslySetInnerHTML`.
- Verify updater signatures. Keep the public key in `tauri.conf.json`; store the private
  key in CI secrets.
- Single-instance plugin to prevent concurrent JSON writes.
- Log with `tracing` and rotation; do not log secrets.

## Commands and events

The exact names can change, but the surface should stay small:

| Command | Purpose |
|---|---|
| `bootstrap` | Settings, locale, theme, library, running state |
| `database_sync` | Download/refresh the detectable catalog |
| `database_page` | Filter and paginate the catalog |
| `library_list` | Read the library |
| `library_add` | Add a database game |
| `library_remove` | Delete a library entry |
| `library_toggle_favorite` | Toggle favorite |
| `library_update_exe` | Override executable name |
| `game_launch` | Copy the dummy and spawn it |
| `game_stop` | Stop one game |
| `game_stop_all` | Stop all games |
| `game_list_running` | Reconcile UI state |
| `steam_detect` | Return Steam path/status |
| `steam_setup` | Write manifest and dummy |
| `steam_remove` | Remove manifest and folder |
| `shortcut_create` | Create a desktop shortcut |
| `settings_get` / `settings_set` | Read/write settings |
| `theme_get` | Current theme/accent/material |
| `update_check` / `update_install` | Updater flow |
| `window_minimize` / `window_toggle_maximize` / `window_close` | Window controls |
| `logs_open` / `diagnostics_copy` | Support actions |

Events: `game://exited`, `game://state`, `quest://tick`, `quest://phase`,
`theme://changed`, `update://progress`, `log://entry`, `library://changed`.

Use `tauri-specta` to generate TypeScript bindings and fail CI if the generated file is
stale.

## Build and release

- Frontend: Vite + React + TypeScript; `pnpm`.
- Rust: stable MSVC toolchain; `cargo fmt`, `clippy -D warnings`, `cargo test`.
- Tauri CLI 2.
- NSIS per-user installer; standalone portable executable via `--no-bundle`.
- App identifier: `com.morphy137.dummies`.
- Product name: `Dummies`.
- Version: start at `1.0.0` for the Rust rewrite.
- Release assets: `Dummies-Setup-<version>.exe`, `Dummies-Portable-<version>.exe`,
  `latest.json`, signatures, checksums.
- Tauri updater: signed artifacts; GitHub Releases endpoint on the canonical repo.
- Portable: manual update via the releases page (same policy as the current app).
- CI on `windows-latest`: install Rust/Node/pnpm, cache, run checks, build, sign,
  upload release.
- Keep the existing Electron/Discord notification workflows until the Rust release
  workflow replaces them.

## Testing

- Unit: version comparison, timer phases, path validation, settings migration, cover
  URL priority, ACF generation, sanitization.
- Integration: temp data dir; add/remove library; copy dummy; launch/stop a test stub;
  Steam setup/remove against a temp Steam tree; atomic writes.
- Frontend: Vitest + Testing Library with mocked IPC; locale key parity.
- E2E (optional): tauri-driver + EdgeDriver for critical flows.
- Manual: Windows 10/11, light/dark, transparency on/off, high contrast, 100–200% DPI,
  multi-monitor, offline, Steam missing, permissions denied, multiple games, tray,
  auto-stop, updater.
- Resource benchmark: sum private working set of the app process tree (including
  `msedgewebview2.exe`) with 0, 1, and 3 running dummies. Compare against the Electron
  baseline.

## Acceptance criteria

- Full functional parity with v0.2.1.
- Migration imports old `myGames.json` and `settings.json` without loss.
- Installer/portable runs without admin for normal use.
- Dark/light, system accent, optional Mica/Acrylic; Windows 10 solid fallback.
- Custom title bar works: minimize, maximize/restore, close, double-click, snap.
- English/Spanish, system default, English fallback, key parity.
- MIT license and third-party notices.
- App visible ≤ 120 MB private working set; hidden in tray ≤ 90 MB; dummy hidden < 5 MB
  each.
- Installer/portable ≤ 15 MB; installed disk ≤ 30 MB.
- No orphan dummies after quit; no path traversal; signed updates.
- CI green: format, clippy, Rust tests, frontend lint/typecheck/tests, Tauri build.

## Phases

| Phase | Scope | Exit criteria |
|---|---|---|
| 0. Spike | Tauri window, Raycast shell, Mica, one poster grid, memory baseline | Go/no-go on Tauri and memory |
| 1. Design | 5 screens × light/dark, tokens, motion | Design approved |
| 2. Scaffold | Cargo workspace, Vite/React, typed IPC, CI | Builds and tests pass |
| 3. Core | Paths, migration, settings, database, library | Core tests pass |
| 4. Dummy | Rust Win32 dummy, hidden/visible, stdin | < 5 MB hidden; copy works |
| 5. Processes/tray/quest/Steam | Launch/stop, tray, timers, Steam, shortcuts | Functional parity backend |
| 6. UI parity | Library, detail, add, settings, activity | Full flows work |
| 7. Polish | Materials, themes, motion, accessibility | QA visual pass |
| 8. Release | NSIS, portable, updater, signing, CI | Signed release candidate |
| 9. Cutover | Migration test, QA, docs, legacy removal | 1.0.0 |

## Toolchain status

Checked on 2026-10-21:

- Node.js: `v26.3.1`.
- pnpm: `12.14.0`.
- Rust: `1.98.1` (`x86_64-pc-windows-msvc`).
- Cargo: `1.98.1`.
- Tauri CLI: `2.12.4`.
- Visual Studio Build Tools 2022 with the C++ workload: present.
- Windows SDK: `10.0.26100.0`.
- WebView2 Runtime: `155.0.4283.62`.
- .NET: present, only needed for the legacy app during transition.

The Tauri shell is scaffolded and the frontend/Rust checks pass. The next work item is
the production memory benchmark and the Rust dummy process.
