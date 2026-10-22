# dummy-game

The tiny process that Dummies copies and renames to the executable name Discord expects.

- Rust + raw Win32 (`windows-sys`).
- No .NET, no WebView2, no Node, no sidecar DLLs.
- Hidden message-only window by default; the launcher tray is the control surface.
- Optional visible window with `--window`.
- stdin commands: `SHOW`, `HIDE`, `QUIT`, `STOP`.
- `--no-tray` keeps the visible window in the taskbar when minimized.

## Build

```powershell
cargo build --release --manifest-path crates/dummy-game/Cargo.toml
```

The release profile uses size optimization, LTO, abort-on-panic, symbol stripping, and
a statically linked CRT.

## Measured baseline

| Metric | Value |
|---|---:|
| Executable size | 134.5 KB |
| Hidden private working set | 0.7 MB |
| Hidden private commit | 1.1 MB |
| Visible private working set | 1.4 MB |
| Visible total working set | 14.6 MB |

## Protocol

The launcher writes newline-delimited commands to the dummy's stdin:

| Command | Effect |
|---|---|
| `SHOW` | Create or show the optional visible window |
| `HIDE` | Destroy the visible window |
| `QUIT` / `STOP` | Exit the process |

The dummy exits on stdin EOF, so a crashed launcher does not leave orphan processes
once the launcher's Job Object is wired up.
