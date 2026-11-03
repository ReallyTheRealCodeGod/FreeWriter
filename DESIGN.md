# FreeWriter — design

A distraction-free writing app for Windows that works like an old typewriter:
set a goal, and you're locked onto a blank page until you reach it.

## Stack

Rust, with egui/eframe for the window (version pinned in `Cargo.toml`).
Windows only. Rust toolchain: the GNU host (`x86_64-pc-windows-gnu`), which
needs no admin rights and no Visual Studio.

The icon and version details (the name "FreeWriter" in Explorer and Task
Manager) are built into the exe. `build.rs` writes them as a `.res` file
itself, which the linker takes directly, so no resource compiler (rc.exe or
windres) is needed. The same icon file gives the windows their icon at run
time.

`install.ps1` installs it per user, without admin rights: the exe in
`%LOCALAPPDATA%\Programs\FreeWriter`, a Start menu shortcut, and an entry under
Settings > Apps that runs `uninstall.ps1`.

## Flow

1. **Start** — a single line: type a number for words (`500`) or minutes
   (`25m`), press Enter. The line below confirms the goal as you type, and
   goals are capped (10,000 words, 240 minutes) so a typo can't lock you in
   for days. An empty line suggests the last goal, or the rest of an
   unfinished session (see [the logbook](#3-the-logbook)). The session goes
   on a new, blank page, or on an earlier one chosen in
   [the menu](#menu-before-a-session).
2. **Writing** — locked until the goal is met.
3. **Goal reached** — the progress line completes and a quiet note says what
   you did and that you can leave (Esc). You can keep writing as long as you
   like, or [read back](#4-reading-back-after-the-goal) through the page.
   Ctrl+S gives a new page its name (see [saving](#saving)).

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
(`%APPDATA%\FreeWriter\settings.toml`); when on, Esc asks for a long fixed
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

- Each page is a plain text file in `Documents\Writing`. Starting FreeWriter
  gives a new, blank page. An earlier page can be chosen in the menu and
  written on again; its text stays above the line.
- A new page is saved from the first word, under the time it was begun
  (`2026-11-03 14.05.txt`), until it has a name.
- **Ctrl+S**, once the goal is reached, saves the page you're on. A page
  without a name asks for one first, on one line at the bottom:

      Save as: Short story_
      Enter saves it as Writing\Short story.txt. Esc cancels.

  The file is renamed, and whatever you write after goes into it.
  Characters Windows doesn't allow in file names can't be typed, and a name
  that's taken is refused, so naming never replaces another page. Before
  the goal Ctrl+S does nothing: the page saves itself anyway, and the screen
  stays empty.
- Leaving with Esc after the goal, from a page with writing on it but no
  name, brings up the same line once: Enter names it and leaves, Esc leaves
  it under its time. It's in the menu either way.
- A page whose name is only a date counts as unnamed: a new page not named
  yet, or a day's page from before pages had names (`2026-10-29.txt`).
- Autosave every few seconds and on exit.
- Atomic writes (write to a temp file, then rename), so a crash or a
  Task Manager kill loses at most a few seconds.
- Each session also gets a line in the app's own log, kept apart from the
  pages (see [the logbook](#3-the-logbook)).

## Look

Courier New (ships with Windows; loaded from the system fonts folder, not
bundled) in black on a white page. Nothing else on screen.

## Menu (before a session)

A way back to what you've written, without leaving the typewriter. It only
exists on the start screen; once a session starts, there's no menu and no
mouse.

- **Opening it:** a faint `Menu` in the top-left corner of the start screen.
  Click it (or press Tab). The mouse pointer shows on the start screen and
  in the menu, and is hidden again while writing.
- **The list:** every page, the one written on last at the top, with when
  that was and its word count, in the same Courier as the page:

      Menu

      Your pages
        Short story                   Today          612 words
        2026-11-02 21.40              Yesterday      140 words
        2026-10-29                    29 October      88 words

      Open the Writing folder

      Esc to go back

  Click a line, or move with Up/Down and press Enter. The line under the
  pointer (or the keys) gets the same faint bar as the typing caret.
  Days within the last week are named (Today, Yesterday, Thursday); older
  ones show the date. The wheel scrolls a long list. Copies saved in
  `Writing\Saved` by an earlier version are listed too.
- **Reading a page:** the page opens on the white paper, read-only, from the
  top. The wheel, PageUp/PageDown, Up/Down and Home/End scroll. Nothing can
  be typed into it. Along the bottom: `Write on it · Back`. Esc goes back to
  the list.
- **Write on it** (or Enter while reading) goes back to the start line with
  that page chosen; the line at the bottom names it. The session continues
  the page on a new paragraph, and the words count from there, as always.
  `New page` beside `Menu` (or Esc) goes back to a blank page.
- **Open the Writing folder** opens it in Explorer, for anything else
  (printing, opening in Word, copying, deleting).
- Esc from the list goes back to the start line, with whatever goal you'd
  typed still there.

## Onlookers (Ctrl+Space)

For when someone walks up and the page is nobody's business. Ctrl+Space
swaps the screen for an Ubuntu terminal under Windows Terminal, busy with
routine admin work; Ctrl+Space again brings back exactly what was there.
It works on every screen: the start line, the menu and a session.

- **The look** is the WSL Ubuntu profile as Windows Terminal draws it,
  full screen: the Ubuntu colour scheme (aubergine `#300A24`, white text),
  Ubuntu Mono at 13 pt, 8 px padding, a white block cursor that blinks when
  nothing is printing. The prompt is the usual Ubuntu one,
  `user@HOST:~$` in green and blue, with the Windows user name in lower
  case and the computer's name, which is what WSL shows. The window's title
  becomes `Ubuntu`, so Alt+Tab agrees. The pointer shows, as an I-beam.
- **The main screen** runs `apt` by itself, nothing to type: `apt update`,
  `apt upgrade`, then installing build tools, Redis, fail2ban and the
  Prometheus node exporter, with a quick `systemctl status` or `df -h` in
  between, and idle pauses at the prompt. The output follows Ubuntu 24.04:
  `Get:` lines, the download status line, `Unpacking`/`Setting up`, the
  green `Progress: [ 37%]` bar along the bottom while dpkg runs, and
  needrestart's report at the end. After the first round it purges what it
  installed and installs it again, as when testing a setup script, and
  `apt update` finds nothing new.
- **The other monitors**, during a locked session, show a terminal each
  instead of blank paper: `htop` on the first (the real number of cores,
  updating every 1.5 seconds, sorted by CPU) and `journalctl -f` on the
  second, with log lines stamped with the real time. More monitors
  alternate the two.
- **Time stands still underneath.** The terminal only runs while it's
  showing, and continues where it left off next time. Keys other than
  Ctrl+Space do nothing: nothing reaches the page or the menu, and Esc
  doesn't quit. A session stays locked, saves as usual, and the time spent
  behind the terminal doesn't count towards a time goal or the log's
  minutes.
- When it first appears, there's already history on screen (`uptime`,
  `df -h`, `free -h`) and the update under way, as if it had been running
  for a while.

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

- **Where:** `%APPDATA%\FreeWriter\log.tsv`, next to `settings.toml`. The
  Writing folder stays your prose only, and nothing that globs `*.txt` there
  picks up bookkeeping. It's a plain tab-separated file, readable in a text
  editor or a spreadsheet:

      date        start  goal  words  minutes  result
      2026-10-22  09:14  500   523    27       reached

  `result` is `reached`, `left early` (the panic sentence) or `unfinished`
  (the session ended before its goal, e.g. through Task Manager or a crash).
- **The session file:** while a session runs, its log line is also written
  to `%APPDATA%\FreeWriter\session.tsv` on every autosave, with the page it's
  on. Leaving properly
  (Esc or closing after the goal, or the panic sentence) moves the line into
  the log and deletes the file. If the file is still there when FreeWriter
  starts, the last session didn't end properly: its line goes into the log
  as it stood at the last autosave (at most a few seconds old).
- **Unfinished sessions come back.** If that leftover session was today and
  short of its goal, the start line offers the rest of it:

      Goal: 188_
      Unfinished from 09:14: 188 words to go. Enter to finish it.

  That's a fresh session with the remaining goal (words still to write, or
  minutes still to go), on the same page. Typing a different goal works as usual. The offer
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
back through the whole page, so you can read what you wrote. It's
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
