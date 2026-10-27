# Writer — design

A distraction-free writing app for Windows that works like an old typewriter:
set a goal, and you're locked onto a blank page until you reach it.

## Stack

Rust, with egui/eframe for the window (version pinned in `Cargo.toml`).
Windows only. Rust toolchain: the GNU host (`x86_64-pc-windows-gnu`), which
needs no admin rights and no Visual Studio.

The icon and version details (the name "Writer" in Explorer and Task
Manager) are built into the exe. `build.rs` writes them as a `.res` file
itself, which the linker takes directly, so no resource compiler (rc.exe or
windres) is needed. The same icon file gives the windows their icon at run
time.

`install.ps1` installs it per user, without admin rights: the exe in
`%LOCALAPPDATA%\Programs\Writer`, a Start menu shortcut, and an entry under
Settings > Apps that runs `uninstall.ps1`.

## Flow

1. **Start** — a single line: type a number for words (`500`) or minutes
   (`25m`), press Enter. The line below confirms the goal as you type, and
   goals are capped (10,000 words, 240 minutes) so a typo can't lock you in
   for days. An empty line suggests the last goal, or the rest of an
   unfinished session (see [the logbook](#3-the-logbook)).
2. **Writing** — locked until the goal is met.
3. **Goal reached** — the progress line completes and a quiet note says what
   you did and that you can leave (Esc). You can keep writing as long as you
   like, or [read back](#4-reading-back-after-the-goal) through the day.

Word goals count words written *this session*, so earlier text doesn't count.
Time goals are wall-clock minutes.

## Lock (medium)

- Kiosk fullscreen, always on top, on the primary monitor. Every other
  monitor gets a blank cover (see [every screen covered](#5-every-screen-covered)).
- Closing and Alt-F4 are refused until the goal is met.
- If the window loses focus, it takes focus back.
- Ctrl+Alt+Del can't be blocked and is always the real-world emergency exit.
  The app must not fight Task Manager for focus.

## Escape hatch

Off by default. Turned on in a small settings file
(`%APPDATA%\writer\settings.toml`); when on, Esc asks for a long fixed
sentence, and typing it exactly lets you leave early.

## Editor

- You type only at the end of the text: no mouse, no arrow keys, no selection.
  Pasting does nothing.
- Enter starts a new line. Backspace erases the last letter, but only back to
  where this session began; earlier sessions are never touched. With
  `backspace = false` it doesn't erase at all.
- Typewriter scrolling: the line you're typing stays fixed at the vertical
  centre and the page moves up as you write.
- Lines wrap softly at about 65 characters.
- Progress is a thin line along the bottom edge, no numbers.
- Danish letters (æ ø å) and dead-key accents must work.

## Saving

- One file per day: `Documents\Writing\YYYY-MM-DD.txt`. A second session on
  the same day continues the same file, with the earlier text above the line.
- Autosave every few seconds and on exit.
- Atomic writes (write to a temp file, then rename), so a crash or a
  Task Manager kill loses at most a few seconds.
- Each session also gets a line in the app's own log, kept apart from the
  pages (see [the logbook](#3-the-logbook)).

## Look

Courier New (ships with Windows; loaded from the system fonts folder, not
bundled) in black on a white page. Nothing else on screen.

## Out of scope

Website/app blocking (that's Cold Turkey *Blocker*), formatting, file picker,
settings screen, editing earlier text, dark mode, typewriter sounds.

## Version 2 plan

**Goal:** make the page harder to escape and easier to come back to, without
adding anything to the screen while you write. Every feature below stays out
of the writing moment: they act before a session, after the goal, or not on
screen at all. Done means each one works, has a headless test, is described
in this file and the README, and the release build is rebuilt.

Work order, safest first. Each step gets its tests before the next starts:

1. Pure typewriter option (done)
2. Numbers once the goal is reached (done)
3. The logbook, which also brings back unfinished sessions, remembers your
   goal and gives a quiet record on the start screen (done)
4. Reading back after the goal (done)
5. Every screen covered (done; last, since only a real window can prove it,
   and that check on a real second monitor is still to do)

### 1. Pure typewriter (`backspace = false`)

A real typewriter can't erase. A new line in `settings.toml` turns Backspace
off for the page. It still works on the start line and in the panic
sentence. The default stays as it is (Backspace on).

### 2. Numbers once the goal is reached

While writing, there are still no numbers, only the thin line. Once the goal
is reached, the note at the bottom says what you did and keeps counting if
you go on:

    Goal reached: 523 words in 27 minutes. Esc to leave, or keep writing.

This is a deliberate exception to "no numbers". By then the session is
already won, so a count can't pull you out of it.

### 3. The logbook

One line per session, kept by the app, so it can tell you where you left off.

- **Where:** `%APPDATA%\writer\log.tsv`, next to `settings.toml`. The
  Writing folder stays your prose only, and nothing that globs `*.txt` there
  picks up bookkeeping. It's a plain tab-separated file, readable in a text
  editor or a spreadsheet:

      date        start  goal  words  minutes  result
      2026-10-22  09:14  500   523    27       reached

  `result` is `reached`, `left early` (the panic sentence) or `unfinished`
  (the session ended before its goal, e.g. through Task Manager or a crash).
- **The session file:** while a session runs, its log line is also written
  to `%APPDATA%\writer\session.tsv` on every autosave. Leaving properly
  (Esc or closing after the goal, or the panic sentence) moves the line into
  the log and deletes the file. If the file is still there when Writer
  starts, the last session didn't end properly: its line goes into the log
  as it stood at the last autosave (at most a few seconds old).
- **Unfinished sessions come back.** If that leftover session was today and
  short of its goal, the start line offers the rest of it:

      Goal: 188_
      Unfinished from 09:14: 188 words to go. Enter to finish it.

  That's a fresh session with the remaining goal (words still to write, or
  minutes still to go). Typing a different goal works as usual. The offer
  is made once, at the first start after the session was cut: by then it's
  in the log, and quitting or choosing another goal lets it go. An
  unfinished session from an earlier day is logged but not offered.
- **Your goal is remembered.** Otherwise the start line suggests the goal of
  the last session, shown faint. Enter takes it, and typing replaces it.
  Taking up an unfinished session comes first.
- **A quiet record.** One faint line near the bottom of the start screen,
  only there (never while writing):

      Yesterday: 523 words · 4 days in a row

  It says `Today so far` if you've already written today, or
  `Last time, 12 October` after a gap. Words are every session's words that
  day. A day counts towards "in a row" if at least one session reached its
  goal. A run is shown from 2 days, ending today or yesterday.
- Starting with `--goal` skips the start line, so no offer is made. A
  leftover session file is still moved into the log.

### 4. Reading back after the goal

Until the goal, the page only goes forward and you can't scroll back to
fiddle. Once the goal is reached, the mouse wheel and PageUp/PageDown scroll
back through the whole day's page, so you can read what you wrote. It's
read-only: there's still no cursor to place. Typing anything (or End)
snaps back to the end, with the typing line in the centre again.

### 5. Every screen covered

During a locked session, each monitor other than the page's gets a plain
paper-coloured window: no text, no cursor, always on top, not on the
taskbar. The page itself opens on the primary monitor.

- The covers follow the page. When Task Manager comes in front, they drop
  to normal level along with the page, so Task Manager can be used on any
  monitor. When the goal is reached (or the panic sentence is typed), they
  close.
- Clicking a cover is like clicking anywhere else: the page takes focus
  back. Closing a cover is refused, like closing the page. Covers open
  without taking focus from the page.
- If a monitor is plugged in or out mid-session, the numbering can shift.
  The covers are dropped rather than risk one landing on the page; that
  fails open, never shut.
- Built with egui's `with_monitor(index)`, which opens a borderless
  fullscreen window on that monitor. The index is the order Windows lists
  monitors in (`EnumDisplayMonitors`), the same order egui uses, so no
  pixel or DPI arithmetic is needed.
- `--windowed` (development) has no covers. Tests swap the monitor list for
  a made-up one, the way they already stand in for Task Manager.
- Only a real window on a real second monitor can confirm it works. That
  check is a manual one.
