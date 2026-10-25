# Review implementation status

2026-10-25, review fixes based on `59f7171`. Recorded locally on `development`; no publication or deployment performed.

The functional safety/UI/distribution pass is commit `954cbef` (`fix: harden Steam integration and managed dummy workflows`).
The subsequent backend-refactor commit separates the implementation without adding dependencies or changing IPC names.

## Implemented

- Steam: strict path validation, reparse-point rejection, registered-library selection,
  dedicated destinations, ownership hashes, confirmed file-only removal, rollback,
  original executable restoration, stable library identity and correct launch target.
- Running games cannot be deleted or reconfigured. Duplicate persisted identities are rejected.
- Dummy lifecycle: timer cancellation on drop, generation/clock checks at completion,
  single-instance launcher, managed shortcuts, standalone stdin behavior and tray restoration.
- Persistence: errors instead of silently empty libraries, flushed unique temporaries,
  backups, operation serialization, validated catalog replacement and per-file migration.
- UI: actionable list, working detail action/Ctrl+K, bounded catalog pages, nonblocking cached
  searching, stale-response protection, distinct empty/error/loading states, timer dock,
  explicit Save/Cancel, imported options, focus restoration, responsive columns and keyboard support.
- Localization/accessibility: document language, translated errors/native copy, high-contrast
  styles, selectable data, larger icon targets and delayed subscription cleanup.
- Security/distribution: CSP, removed opener, release logging, update install guard,
  portable marker/manual updates, root static-CRT config, packaged license inventories,
  CI checks before release and truthful architecture/QA documentation.
- Backend: extracted models/state, settings, library/catalog/bootstrap, storage/path validation/migration,
  Steam, binary resources/targets, process lifecycle/timers, tray and shortcuts. `lib.rs` now composes
  the app; `commands.rs` contains thin IPC adapters. Services borrow `AppState` directly, making
  library identity, running-entry protection and concurrent mutations testable without a webview.
- Contract regression coverage: Rust request deserialization/result serialization and frontend command
  argument tests, including nullable exit codes, timer snapshots and unknown persisted fields.

## Local verification

| Command/check | Result |
| --- | --- |
| `pnpm check` | Pass: Biome and TypeScript |
| `pnpm test` | 28 tests pass |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml` | 28 tests pass |
| `cargo test --locked --manifest-path crates/dummy-game/Cargo.toml` | 5 tests pass |
| `cargo fmt --manifest-path <each Cargo.toml> -- --check` | Both pass |
| `cargo clippy --locked --manifest-path <each Cargo.toml> --all-targets -- -D warnings` | Both pass |
| `pnpm tauri build --no-bundle` | Release executable builds |
| `pnpm package:portable` | ZIP builds, 6.49 MiB; required resource inventory verified |
| `powershell -ExecutionPolicy Bypass -File scripts/test-dummy.ps1` | SHOW/HIDE/QUIT pass, exit 0 |
| Standalone native dummy launch/CloseMainWindow | Survives missing stdin and closes normally |
| MSVC `dumpbin /dependents` on release dummy | System DLLs only; no VC runtime DLL dependency |
| Browser preview | Settings language preview, Escape reversion, Ctrl+K and overflow checked; no console warnings/errors |
| `git diff --check` | Pass |
| Normalized extraction comparison against `954cbef` | All 77 original top-level functions retained; bodies unchanged except state borrowing/registration plumbing and formatting |
| Command registration parity | Same 24 names, each registered exactly once |

The refactor reruns frontend checks/tests, both Rust test suites, Rustfmt/Clippy and the release/portable
builds. Native protocol, standalone behavior, DLL inspection and browser preview rows describe the
earlier safety-pass smoke checks, not additional manual qualification of the refactor.

The current static-CRT dummy is 933,888 bytes (912 KiB). Private working set in the
protocol smoke test: 3.6 MB initially hidden, 4.0 MB visible/hidden after showing.
The launcher process tree has **not** been rebenchmarked; the old ~147 MB result is historical.
Vite reports a nonfatal ~568 kB minified entry chunk; chunk splitting remains an optimization opportunity.

## Not certified as complete

- Real Discord detection and quest acceptance, live Steam behavior, Windows 10/11/DPI/multi-monitor
  qualification and signed installer upgrades require the manual checks in `QA.md`.
- The complete launcher memory target is not demonstrated by the dummy-only smoke test.
- The generated inventory contains 421 dependencies, including a conservative build/platform
  superset. 13 packages have no license text in their installed package and are explicitly marked
  for manual review. This is not a declaration of complete legal compliance.
- Generated IPC bindings and a dedicated platform-independent app-core crate have not been introduced.
  Contract regression tests reduce drift but do not replace generated bindings or end-to-end IPC testing.
- Multi-file Steam rollback handles ordinary errors, not arbitrary power loss or a privileged
  local process racing filesystem operations. Unverifiable legacy files are preserved for inspection.
- Automated CI is configured but has not been executed remotely. No installer was installed and
  no real user Steam library was modified during validation.
