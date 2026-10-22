# Writer

A distraction-free writing app for Windows that works like an old typewriter.
Set a goal, and the page holds you until you reach it. Why it works the way it
does is in [DESIGN.md](DESIGN.md).

## Build

Needs Rust (the GNU toolchain is fine; no Visual Studio needed).

```powershell
cargo build --release
```

The app is `target\release\writer.exe`. Copy it anywhere you like, or pin a
shortcut to it.

## Writing

1. Start the app. Type a goal and press Enter:
   - `500`: write 500 words (at most 10,000)
   - `25m`: write for 25 minutes (at most 240)

   The line under it confirms what you typed, e.g. *25 minutes*. Esc quits.
2. Write. Text only goes in at the end; Backspace takes back letters typed in
   this session. The line you're typing stays in the middle of the screen, and
   the thin line along the bottom shows how far you've come.
3. When the goal is reached, a note appears. Press Esc to leave, or keep going.

Until then, the window stays in front and won't close.

To skip the start line, give the goal when starting, e.g. in a desktop
shortcut: `writer.exe --goal 500` or `writer.exe --goal 25m`.

## Your pages

One file per day: `Documents\Writing\2026-10-21.txt`. Writing twice in a day
continues the same file on a new paragraph. The page is saved every few
seconds, so even a forced exit loses at most a few seconds of writing.

To keep the pages somewhere else, start it with `--pages`, e.g.
`writer.exe --pages D:\Notes`.

## Getting out early

- **Emergency:** Ctrl+Alt+Del → Task Manager → select Writer → End task. The
  app steps aside for Task Manager; your page is already saved.
- **Panic exit (optional):** create `%APPDATA%\writer\settings.toml` containing

  ```toml
  panic_exit = true
  ```

  Then Esc during a session asks you to type a long sentence; typing it exactly
  and pressing Enter lets you leave.

## Limits

- With several monitors, only one is covered; the others stay visible, though
  the page takes focus back as soon as you click elsewhere.
- Win+L still locks the computer, and Ctrl+Alt+Del can't be blocked (on purpose).

## Development

- `cargo run -- --windowed` runs in a normal window without the lock.
- `writer.exe --goal 15s` locks for just 15 seconds: a quick way to try the
  lock itself (goals also accept seconds).
- `cargo test` runs the unit tests.
- `cargo test -- --ignored screenshots` renders the screens to
  `target\screenshots` (needs a graphics card).
