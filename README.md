# Dummies — Rust/Tauri migration

This folder is the isolated home of the next version of the launcher. The existing
Electron app remains in `../discord-dummies-launcher/electron/` and the legacy C#
projects remain in `../discord-dummies-launcher/src/`. Nothing outside this folder is
modified by the migration until feature parity is verified.

## Status

The Rust/Tauri rewrite now covers the core launcher flow: real Discord catalog, a
persistent library, legacy data migration, launch/stop, tray, quest timers,
notifications, shortcuts, Steam manifests, and a manual GitHub update check.

Verified locally:

- `pnpm check` / `pnpm build` / `pnpm test`
- `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test`
- `pnpm tauri build --no-bundle` and NSIS packaging
- End-to-end launch, auto-stop, favorite, edit executable, delete and shortcut flows

Pending: signed auto-updater, portable packaging, code signing, and broader QA.

### Release shell baseline

Measured on 2026-10-21 with the release build, a real 28-entry library, window visible
at 1180×760:

| Metric | Value |
|---|---:|
| App process private working set | 14.3 MB |
| WebView2 process tree private working set | ~132 MB |
| Total private working set (Task Manager-style) | ~147 MB |
| Total private commit | ~236 MB |
| Electron baseline reported by the user | ~160 MB |
| NSIS installer | 3.79 MiB |
| Portable ZIP | 5.02 MiB |
| Release app executable | 14.22 MiB |

The Rust shell itself is tiny; WebView2 is the remaining cost. The dummy process
rewrite is the larger resource win, because the current C# dummy carries a .NET
runtime per game.

Use `pnpm measure` while the app is running to reproduce the process-tree measurement.

### Dummy process baseline

The Rust/Win32 dummy is intentionally tiny:

| Metric | Value |
|---|---:|
| `dummy-game.exe` size | 134.5 KB |
| Hidden private working set | 0.7 MB |
| Hidden private commit | 1.1 MB |
| Visible private working set | 1.4 MB |
| Visible total working set | 14.6 MB |

Build and test it with:

```powershell
cargo test --manifest-path crates/dummy-game/Cargo.toml
cargo build --release --manifest-path crates/dummy-game/Cargo.toml
```

This replaces the self-contained C# WinForms template, which carried a .NET runtime
per game and could use tens of megabytes for each dummy.

- Product name: **Dummies**
- App identifier: `com.morphy137.dummies`
- Stack: Tauri 2 + Rust + React/TypeScript + Fluent UI v9
- Dummy process: Rust/Win32, hidden by default, no .NET
- Design: Raycast-inspired, dark-first, light adaptation, optional Mica/Acrylic
- Languages: English and Spanish, system default, English fallback
- License: MIT (see `LICENSE` and `THIRD_PARTY_NOTICES.md`)

## Testing

### Run the current app

From this folder:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
pnpm build:dummy
pnpm tauri:dev
```

To run the already-built release executable:

```powershell
.\src-tauri\target\release\dummies.exe
```

What works today:

- Real Discord detectable catalog, search, paging and `Add game`.
- Persistent `myGames.json` library, with automatic migration from the old Electron
  userData on first run.
- Search, `All / Favorites / Active` filters, grid/list toggle and live covers.
- Favorite toggle, executable override, delete, and desktop shortcut creation.
- Game detail view with cover backdrop, metadata, launch/stop and Steam status.
- Steam manifest setup/removal from the detail view.
- Launch/stop a dummy from a poster or detail view; the Rust dummy is copied into the
  app data `games/` folder and managed by the Tauri backend.
- Tray menu with running games, show/hide, stop, stop all, open and quit.
- Quest countdown, sync buffer, auto-stop and desktop notification.
- System light/dark theme, manual theme override and English/Spanish UI.
- Settings dialog persisted to `%APPDATA%\com.morphy137.dummies\settings.json`.
  Theme and language preview immediately, Cancel reverts, clicking outside saves, and
  a short overlay toast confirms the save without shifting the layout.
- Cards-per-row setting (4–8) applied live to the poster wall.
- Manual GitHub release check with version display.
- Fluent System Icons for app chrome, window controls, cards and toasts.
- Responsive poster wall at the minimum window size.

Pending:

- Signed Tauri auto-updater and code signing.
- Portable packaging.
- Broader QA across Windows 10/11, DPI, high contrast and offline states.

### Test the dummy process

```powershell
powershell -ExecutionPolicy Bypass -File scripts\test-dummy.ps1 -DisplayName "Helldivers 2"
```

The script starts the dummy hidden, sends `SHOW`, measures it, sends `HIDE`, then
`QUIT`. Add `-Visible` to start with the optional window already visible:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\test-dummy.ps1 -DisplayName "Helldivers 2" -Visible
```

### Measure the app process tree

With the app running:

```powershell
pnpm measure
```

This sums the Rust app process and its WebView2 children using Task Manager-style
private working set.

### Test the installer or portable package

```powershell
.\src-tauri\target\release\bundle\nsis\Dummies_1.0.0_x64-setup.exe
```

Or build and use the portable ZIP:

```powershell
pnpm tauri:build -- --no-bundle
pnpm package:portable
.\src-tauri\target\release\portable\Dummies-Portable-1.0.0.zip
```

The installer is unsigned, so Windows SmartScreen may ask for confirmation. It
installs the current Rust/Tauri launcher.

### Test light/dark and language

- Change Windows Settings → Personalization → Colors → “Choose your mode”. The app
  follows the system theme at runtime.
- Open Settings to override Theme and Language manually. “System” uses the OS theme
  and the system locale returned by the Rust backend; missing translations fall back
  to English.
- Dark is the canonical Raycast surface; light is an adapted variant. The current
  system theme determines which one opens.

## Read first

- [`docs/DESIGN.md`](docs/DESIGN.md) — visual system, tokens, screens, motion, copy.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — stack, workspace, dummy
  optimization, data migration, commands, security, phases.

## Planned layout

```text
dummies-launcher/
  Cargo.toml
  crates/
    app-core/
    dummy-game/
  src-tauri/
  src/
  docs/
```

## Prerequisites

1. Visual Studio 2022 Build Tools with “Desktop development with C++” and the Windows
   10/11 SDK.
2. Rust stable MSVC (`rustup default stable-x86_64-pc-windows-msvc`).
3. Node.js LTS and pnpm.
4. The Tauri CLI 2 (installed as a project dev dependency).
5. The WebView2 runtime (present on Windows 11 and most Windows 10 systems).

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full setup and release plan.

## Boundaries

- Do not add `Discord` to the window title, installer title, tray tooltip, or main
  navigation. The app is called **Dummies**.
- The old Electron app is the reference implementation, not a dependency of the Rust
  app.
- Do not delete or rewrite the old app/data. Import a copy of the old data on first run.
- Do not introduce a second primary CTA color. Dark mode uses a white primary button;
  light mode uses a black one.
- Do not apply Acrylic to the main content surface. Mica is optional and limited to the
  title bar/backdrop.
