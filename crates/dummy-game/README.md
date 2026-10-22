# dummy-game

The tiny process that Dummies copies and renames to the executable name Discord expects.

- Rust + raw Win32 (`windows-sys`).
- No .NET, WebView2, Node, or sidecar DLLs.
- Embedded Mood SVG frames, selected randomly on every launch.
- Transparent layered window with a custom minimize/close frame.
- Animated blink using open-eye and closed-eye frames.
- A small normal detection window sits under the face so Discord's window scan
  still sees a conventional titled window.
- Visible by default; `--hidden` starts it hidden.
- stdin commands: `SHOW`, `HIDE`, `QUIT`, `STOP`.
- `--no-tray` keeps the visible window in the taskbar when minimized.

## Build

```powershell
cargo build --release --manifest-path crates/dummy-game/Cargo.toml
```

The release profile uses size optimization, LTO, abort-on-panic, symbol stripping, and
a statically linked CRT.

## Art frames

The source SVGs and their pre-rendered transparent frames live in `assets/` and
`assets/frames/`. Each mood has an open-eye and a closed-eye frame at 320x320. The
dummy embeds the frames at compile time and draws them through a layered window using
`UpdateLayeredWindow`.

## Measured baseline

| Metric | Value |
|---|---:|
| Executable size | 570 KiB |
| Window | 320x356 transparent layered |
| Position | Centered at launch |
| Controls | Minimize and close only |

## Protocol

The launcher writes newline-delimited commands to the dummy's stdin:

| Command | Effect |
|---|---|
| `SHOW` | Restore and focus the window |
| `HIDE` | Hide the window |
| `QUIT` / `STOP` | Exit the process |

The dummy exits on stdin EOF, so a crashed launcher does not leave orphan processes.
