# Writer — design

A distraction-free writing app for Windows that works like an old typewriter:
set a goal, and you're locked onto a blank page until you reach it.

## Stack

Rust, with egui/eframe for the window (version pinned in `Cargo.toml`).
Windows only. Rust toolchain: the GNU host (`x86_64-pc-windows-gnu`), which
needs no admin rights and no Visual Studio.

## Flow

1. **Start** — a single line: type a number for words (`500`) or minutes
   (`25m`), press Enter. The line below confirms the goal as you type, and
   goals are capped (10,000 words, 240 minutes) so a typo can't lock you in
   for days.
2. **Writing** — locked until the goal is met.
3. **Goal reached** — the progress line completes and a quiet note says you can
   leave (Esc). You can keep writing as long as you like.

Word goals count words written *this session*, so earlier text doesn't count.
Time goals are wall-clock minutes.

## Lock (medium)

- Kiosk fullscreen, always on top.
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
- Enter starts a new line. Backspace erases the last letter, but only back to
  where this session began; earlier sessions are never touched.
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

## Look

Courier New (ships with Windows; loaded from the system fonts folder, not
bundled) in black on a white page. Nothing else on screen.

## Out of scope

Website/app blocking (that's Cold Turkey *Blocker*), formatting, file picker,
settings screen, editing earlier text, dark mode, typewriter sounds.
