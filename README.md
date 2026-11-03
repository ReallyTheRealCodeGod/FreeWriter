# FreeWriter

A distraction-free writing app for Windows and Mac that works like an old
typewriter. Set a goal, and the page holds you until you reach it. Why it
works the way it does is in [DESIGN.md](DESIGN.md).

## Download

- **Windows:** [FreeWriter-Setup.exe](https://github.com/ReallyTheRealCodeGod/FreeWriter/releases/latest/download/FreeWriter-Setup.exe).
  Open it, click **Install**, then **Finish**.
- **Mac** (new, not yet tried much): [FreeWriter.dmg](https://github.com/ReallyTheRealCodeGod/FreeWriter/releases/latest/download/FreeWriter.dmg).
  Open it and drag FreeWriter onto Applications.

The first time, Windows or macOS warns about it, because FreeWriter isn't
signed with a paid certificate:

- **Windows** says "Windows protected your PC": click **More info**, then
  **Run anyway**. If your browser says the file isn't commonly downloaded,
  choose **Keep**.
- **Mac** won't open it: close the message, then open System Settings →
  Privacy & Security, scroll down to "FreeWriter was blocked" and click
  **Open Anyway**.

Needs Windows 10 or 11 (64-bit), or macOS 11 or later. Every version is on
the [releases page](https://github.com/ReallyTheRealCodeGod/FreeWriter/releases).

## Writing

1. Start the app. Type a goal and press Enter:
   - `500`: write 500 words (at most 10,000)
   - `25m`: write for 25 minutes (at most 240)

   The line under it confirms what you typed, e.g. *25 minutes*. Esc quits.
   Your last goal is suggested in grey: Enter takes it, typing replaces it.
   You write on a new, blank page, unless you chose an earlier one in the
   menu (see below).
2. Write. Text only goes in at the end; Backspace takes back letters typed in
   this session. The line you're typing stays in the middle of the screen, and
   the thin line along the bottom shows how far you've come. No numbers until
   the goal.
3. When the goal is reached, a note says how many words and minutes. Press
   Ctrl+S to save the page under a name, Esc to leave, or keep going. The
   mouse wheel or PageUp/PageDown now scrolls back through the page so you
   can read it; typing (or End) goes back to the end.

Until then, the window stays in front and won't close, and your other
monitors are covered with blank paper.

To skip the start line, give the goal when starting, e.g. in a desktop
shortcut: `freewriter.exe --goal 500` or `freewriter.exe --goal 25m`.

## Your pages

Each page is a text file in `Documents\Writing`. A new page is saved every
few seconds from the first word, so even a forced exit loses at most a few
seconds of writing. Until it has a name, it goes by the time you began it:
`2026-11-03 14.05.txt`.

Once the goal is reached, **Ctrl+S** saves the page. If it has no name yet,
it asks for one (`Save as: Short story`), and the page becomes
`Documents\Writing\Short story.txt`; anything you write after goes into it.
A name that's already taken is refused, so nothing gets replaced. Leaving
with Esc asks for a name once if the page hasn't got one; Esc again leaves
it under its time.

To keep the pages somewhere else, start it with `--pages`, e.g.
`freewriter.exe --pages D:\Notes`.

### The menu

Before a session, click **Menu** in the top-left corner (or press Tab) to
see everything you've written, the page you wrote on last at the top, with
when and how many words. Click one (or use Up/Down and Enter) to read it.
It's read-only; scroll with the mouse wheel, the arrow keys or
PageUp/PageDown.

While reading, **Write on it** (or Enter) takes you back to the goal line
with that page chosen: the session carries on at the end of it, on a new
paragraph. **New page**, next to Menu (or Esc), goes back to a blank page.
**Open the Writing folder** opens it in Explorer. Esc goes back a step; the
menu is never there during a session.

## Someone looking?

Press **Ctrl+Space** and the screen turns into an Ubuntu terminal under
Windows Terminal, installing and updating packages by itself. While a
session is locked, your other monitors show `htop` and `journalctl -f`. Press
Ctrl+Space again to come back to exactly where you were. Nothing you type
in between goes anywhere, and the time doesn't count towards a time goal.

## Your log

Every session gets a line in `%APPDATA%\FreeWriter\log.tsv`: date, start time,
goal, words, minutes, and how it ended (`reached`, `left early` or
`unfinished`). It's tab-separated, so it opens in a spreadsheet. The start
screen shows one quiet line from it, e.g. *Yesterday: 523 words · 4 days in a
row* (a day counts if you reached a goal).

If a session ends without reaching its goal and without the panic sentence
(Task Manager, a crash, a power cut), the next start that day offers the rest
of it, on the same page: *Unfinished from 09:14: 188 words to go. Enter to
finish it.* It's offered once: take it, or it's simply in the log.

## Settings

There's no settings screen. Create `%APPDATA%\FreeWriter\settings.toml` with any
of these lines:

```toml
panic_exit = true   # Esc can end a session early (see below)
backspace = false   # a pure typewriter: Backspace doesn't erase
```

## Getting out early

- **Emergency:** Ctrl+Alt+Del → Task Manager → select FreeWriter → End task. The
  app steps aside for Task Manager, on any monitor; your page is already
  saved.
- **Panic exit (optional):** with `panic_exit = true` in the settings, Esc
  during a session asks you to type a long sentence; typing it exactly and
  pressing Enter lets you leave.

## On a Mac

It works the same, with these differences:

- Saving is **Cmd+S**.
- The log and settings are in `~/Library/Application Support/FreeWriter`.
  Pages are in `Documents/Writing`, as on Windows; macOS asks once whether
  FreeWriter may use the Documents folder.
- The lock is gentler: the page is full screen and on top and won't close,
  but it doesn't pull itself back to the front, and your other screens
  aren't covered. **Force Quit** (Cmd+Option+Esc) is the emergency exit.
- There's no Ctrl+Space terminal (on a Mac, Ctrl+Space switches keyboard
  input sources).

## Limits

- The page goes on the primary monitor and the others get blank covers. If a
  monitor is plugged in or out during a session, the covers are taken away
  rather than risk one covering the page.
- Win+L still locks the computer, and Ctrl+Alt+Del can't be blocked (on purpose).

## Building it yourself

Needs Rust (on Windows the GNU toolchain is fine; no Visual Studio needed).

```powershell
cargo build --release
```

The app is `target\release\freewriter.exe`, a single file with its icon built in
(from `assets\freewriter.ico`, by `build.rs`).

### Installing your own build on Windows

```powershell
powershell -ExecutionPolicy Bypass -File install.ps1
```

This builds the release and installs it for you alone (no admin rights):
`freewriter.exe` goes in `%LOCALAPPDATA%\Programs\FreeWriter`, FreeWriter
appears in the Start menu and in Windows search, and it's listed under
Settings > Apps. Add `-Desktop` for a desktop shortcut too, or `-NoBuild` to
install the release that's already built. Run it again after changes to
update; that works on a copy installed with FreeWriter-Setup.exe too.

The app used to be called Writer. Installing FreeWriter takes the old Writer
install away, and the first start moves its log and settings from
`%APPDATA%\writer` to `%APPDATA%\FreeWriter`.

To pin it, right-click FreeWriter in the Start menu and choose *Pin to taskbar*.

To remove it, uninstall FreeWriter from Settings > Apps, or run `uninstall.ps1`.
Your pages, log and settings are kept.

### The installers

GitHub builds them ([.github/workflows/build.yml](.github/workflows/build.yml)):
every push runs the tests on Windows and on a Mac and makes both installers,
and the Windows one is installed and uninstalled again to check it. Pushing a
version tag that matches `Cargo.toml` (e.g. `v1.0.0`) also publishes them as
a release, with [installer/release-notes.md](installer/release-notes.md) as
its text.

- **Windows:** [installer/windows/freewriter.iss](installer/windows/freewriter.iss),
  for Inno Setup 6: `ISCC.exe /DAppVersion=1.0.0 installer\windows\freewriter.iss`
  after a release build makes `target\installer\FreeWriter-Setup.exe`.
- **Mac:** `sh installer/macos/build-dmg.sh` on a Mac makes
  `target/macos/FreeWriter.dmg`, for both Apple silicon and Intel.

## Development

- `cargo run -- --windowed` runs in a normal window without the lock.
- `freewriter.exe --goal 15s` locks for just 15 seconds: a quick way to try the
  lock itself (goals also accept seconds).
- `cargo test` runs the unit tests.
- `cargo test -- --ignored screenshots` renders the screens, the terminal's
  too, to `target\screenshots` (needs a graphics card).
