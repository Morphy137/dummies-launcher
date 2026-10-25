# Dummies — implemented architecture

Updated: 2026-10-25. This document describes the implementation, not the former migration proposal.

## Components

- `src/`: React, TypeScript, Fluent UI, i18next; browser preview has no desktop backend.
- `src/lib/backend.ts`: IPC boundary. Persisted `LibraryGame` and runtime `Game` are separate types.
- `src-tauri/src/lib.rs`: application composition, plugins, state setup, command registration and shutdown.
- `src-tauri/src/commands.rs`: the 24 IPC adapters; existing names, payloads and async dispatch modes are preserved.
- `src-tauri/src/models.rs`: persisted models and IPC/event DTOs without desktop-runtime dependencies.
- `src-tauri/src/state.rs`: shared state, paths and the process-wide mutation/update gates.
- `src-tauri/src/settings.rs`, `library.rs`, `catalog.rs`, `bootstrap.rs`: settings, validated library mutations,
  catalog sync/search and startup/reconciliation snapshots.
- `src-tauri/src/storage.rs`, `paths.rs`, `migration.rs`: atomic JSON/backup operations, Windows-safe path
  validation/canonical containment and non-destructive legacy imports.
- `src-tauri/src/steam.rs`: registered library discovery and fingerprint-verified setup/removal/rollback.
- `src-tauri/src/resources.rs`, `dummy.rs`: binary discovery, template preparation and verified launch targets.
- `src-tauri/src/processes.rs`, `timers.rs`: child lifecycle/monitoring, update exclusion, local clocks and completion.
- `src-tauri/src/tray.rs`, `shortcuts.rs`: native window restoration, tray menus and managed Windows shortcuts.
- `src-tauri/src/tests.rs`, `regression_tests.rs`, `ipc_contract_tests.rs`: unit, isolated filesystem/process,
  service-concurrency and serialized-contract checks; never touches a real Steam installation.
- `src/lib/backend.test.ts`: frontend IPC command/argument regression checks.
- `crates/dummy-game`: native layered Win32 window and embedded PNG blink frames, no WebView.

There is no `app-core` crate, Cargo workspace, generated Specta binding or Windows Job Object.
The backend is split by responsibility inside the existing crate, not into a new dependency graph of crates.
Services take `&AppState` rather than Tauri `State`; runtime-facing services still use `AppHandle` where
events, resources or native menus are required. These are not claimed to be a platform-independent core.
Cross-module APIs are crate-visible, not exported as a public library API. Test modules use explicit imports.

## Data and concurrency

Data lives in `%APPDATA%/com.morphy137.dummies`: `settings.json`, `myGames.json`,
`gamelist.json` and `games/<appId>/<relativeExe>`. Portable means no installer, not
portable user data. Unknown library/settings fields are preserved; Settings carries a schema version.

The single-instance plugin is registered first. A backend operation mutex serializes
library read/modify/write and launch/setup/removal operations. Heavy commands use
Tauri's asynchronous command dispatch; network synchronization has a separate gate.
The frontend also deduplicates pending sync requests. Services retain the original lock scopes;
IPC adapters delegate without acquiring the same mutex again. Timer/file ID counters remain single
statics in their owning modules, not per-command instances.

Missing JSON initializes defaults. Malformed/unreadable library data returns an error;
mutations do not overwrite it. JSON saves flush a uniquely named sibling temporary,
preserve the previous valid contents as `.json.bak`, then rename. Restore backups manually
with Dummies closed after inspecting the affected file. Settings corruption is surfaced at bootstrap.
Legacy directories are scanned per missing file, not stopped after the first partial match;
imports leave originals unchanged and create backups.

Catalog downloads have a 30-second timeout and are parsed/validated before replacing
the cache. Cached searches remain available while refresh runs. The dialog renders one
60-entry page at a time, avoiding unbounded DOM growth without a virtualization dependency.
Closing a dialog ignores stale responses; an already running backend download finishes
or times out in the background. It does not block cached searching.

## Steam safety

Steam is detected through HKCU and standard install locations; additional libraries come
from `steamapps/libraryfolders.vdf`. Only registered library roots are accepted by IPC.
Every integration is addressed by **appId + current executable**, never just appId.

- New integrations require a new dedicated install folder and absent manifest. Real
  installations/manifests are never adopted or overwritten.
- Absolute paths, traversal, reserved Windows names, alternate streams and reparse points
  are rejected. Canonical containment is checked before existing files are accessed.
- SHA-256 ownership fingerprints, original executable and selected library are persisted.
- Updates may replace only unchanged, fingerprint-verified artifacts. To change paths or
  App ID, explicitly remove the integration and configure it again.
- Removal requires confirmation and deletes only verified manifest/executable files.
  Empty directories created by Dummies can be removed; unrelated files are retained.
- Library deletion never silently deletes Steam files. Remove the integration first.
- Launch resolves the verified Steam executable; shortcuts dispatch through the launcher.
- Failed saves attempt rollback. This is not a crash-proof multi-file filesystem transaction:
  keep Steam closed during changes, and inspect residual files after a power failure.
- Legacy integrations without ownership fingerprints are not trusted automatically. Existing
  files are left untouched; back them up/move them manually before removing/recreating the
  integration. Their original pre-Steam executable may be unknown and must be edited manually.

Filesystem checks protect normal operation; they are not a sandbox against a privileged
local process racing filesystem changes. Steam/Discord compatibility is external behavior,
not a guarantee that a quest is accepted or completed.

## Process lifecycle and timers

The launcher copies local templates atomically and compares contents, not just file size.
Each child has piped stdin. `--managed` enables EOF shutdown, so normal exit/crash closes
the pipe; explicit stop sends `QUIT`, then kills/waits if necessary. Removing a child from
the map cancels its timer through `Drop`. Timer completion checks the clock identity so an
old timer cannot stop a newly launched process with the same key.

Closing a **dummy** stops it. Minimizing hides it when configured; right-click the app's
tray icon, choose the running game and **Show / restore window**. Standalone dummy runs
minimize to the taskbar and do not depend on stdin. Shortcuts start/reuse Dummies and
launch a managed child. Minimize the launcher to hide it in the tray; closing it quits
and stops children. No close-to-tray behavior is claimed.

Timer target, buffer, auto-stop and notification options are snapshots at launch.
Settings changes affect new sessions. Pausing pauses only the local clock, not presence.
The card and detail share a framed seven-segment countdown without phase/progress copy.
The card hides the clock on hover or keyboard focus so favorite/pause/stop controls do not overlap it.
The countdown is a local estimate, not verified quest completion.

## UI and security

English/Spanish include native tray/notification copy. Technical backend diagnostics may
remain English. Fluent dialogs manage modal focus; detail restores focus, Escape returns,
Ctrl+K focuses library search, arrow keys navigate cards, and list view supports actions.
Settings previews appearance; Save persists, Escape/backdrop cancels. Failed Save keeps the draft.
Update prompts wait for other dialogs to close and installation requires stopping active dummies.

CSP allows only bundled scripts/fonts, Fluent inline styles, IPC and the explicit Steam/Discord
image hosts, plus blob/data images. No shell or unused opener plugin is exposed. Logs are enabled
in release as well as development; avoid attaching them publicly without reviewing private paths.
Cover Cache Storage is capped at 128 entries, expires after seven days, and inactive object URLs
are revoked after 30 seconds. Failed candidates retry after a bounded session-memory TTL.

Dependency-license texts are collected into `DEPENDENCY_LICENSES.txt` during resource
preparation and included with installer/portable packages. Missing texts are explicitly
marked for review rather than assuming the short license identifier is sufficient.

## Builds, releases and verification

Build on Windows with Rust MSVC, WebView2, Node 22+ and pnpm from `packageManager`.
Root `.cargo/config.toml` enables static CRT for MSVC even with `--manifest-path`.
`pnpm build:dummy` generates the bundled resource. `pnpm tauri build --bundles nsis`
builds the frontend/dummy as prerequisites. Updater signing requires CI/private signing material;
no signing key is stored in the repository. Windows Authenticode signing remains pending.

Checks run on pushes/PRs and before release: Biome/TypeScript, Vitest, frontend build,
Rustfmt, Clippy with warnings denied, and both Rust test suites. Manual workflow releases
also validate the selected tag before signing/publishing. Defining workflows does not publish anything.
Portable ZIPs include `portable.marker`, LICENSE and third-party notices, disable in-app updates
and must be replaced manually from GitHub Releases. Installed builds use signed NSIS updates.

Historical measurements (not revalidated by unit tests): dummy 779 KiB/~3 MB private working set;
shell ~147 MB with 28 games. The former 120 MB shell target is **not met**. Use
`scripts/measure-memory.ps1` to measure the complete process tree, including WebView2, on release
builds. Do not present Rust-process memory alone as total app memory.

Remaining manual qualification: Windows 10/11, 100–200% DPI, high contrast, multiple monitors,
real Discord detection, live Steam library behavior, signed installer upgrade and portable replacement.
See `QA.md` for the checklist. Neither external service acceptance nor all dependency-license
obligations can be established by unit tests alone.
