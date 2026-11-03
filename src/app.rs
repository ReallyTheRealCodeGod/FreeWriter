use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Event, FontId, Key, MouseWheelUnit, Painter, Rect, ViewportCommand,
    WindowLevel, pos2, vec2,
};

use chrono::{DateTime, Local, NaiveTime, Timelike};

use crate::goal::Goal;
use crate::icon;
use crate::lock::{self, Monitors};
use crate::logbook::{self, Entry, Logbook, Outcome};
use crate::menu::{self, Menu};
use crate::page::{self, Listed, Page};
use crate::settings::Settings;
use crate::terminal::{self, Terminal};

const PAPER: Color32 = Color32::WHITE;
pub const INK: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x1a);
pub const FAINT: Color32 = Color32::from_rgb(0x9a, 0x9a, 0x9a);

pub const TEXT_SIZE: f32 = 20.0;
pub const NOTE_SIZE: f32 = 14.0;
/// Width of the column in characters, like a typewriter's margins.
const COLUMN: f32 = 65.0;
/// How much of the end of the page to lay out; plenty to fill the screen.
const TAIL_BYTES: usize = 20_000;
/// How far one notch of the mouse wheel reads back, as egui scrolls.
pub const LINE_SCROLL: f32 = 40.0;

const AUTOSAVE_EVERY: Duration = Duration::from_secs(3);
/// Even with nothing typed, the running session's log line is brought up to
/// date this often, so a time goal's minutes are known if the session is cut.
const LOG_EVERY: Duration = Duration::from_secs(30);
/// How often a locked session checks that it's still in front. Taking focus
/// back fakes an Alt key press, so this shouldn't be too frequent.
const LOCK_TICK: Duration = Duration::from_millis(250);

/// Typing this exactly ends a session early, if `panic_exit` is on.
const PANIC_SENTENCE: &str = "I choose to stop writing now, before I have reached my goal.";

/// How long a page's name can be, in characters.
const NAME_CHARS: usize = 60;
/// How long "Saved as ..." stays at the bottom.
const NOTE_FOR: Duration = Duration::from_secs(4);

pub struct WriterApp {
    settings: Settings,
    /// Development mode: a normal window and no lock.
    windowed: bool,
    /// The folder the pages go in.
    pages: PathBuf,
    /// Where sessions are logged; `None` if there's no app folder.
    logbook: Option<Logbook>,
    screen: Screen,
    /// The window level last asked for, so it's only sent when it changes.
    level: WindowLevel,
    /// Whether Task Manager is in front. A field so tests can stand in for it.
    task_manager_in_front: fn() -> bool,
    /// The monitors as they are now; a field for tests, like the one above.
    monitors: fn() -> Monitors,
    /// The monitors as they were at the start, when the page went on the
    /// primary one.
    placed: Monitors,
    /// The terminal Ctrl+Space puts over everything, for onlookers.
    terminal: Terminal,
}

enum Screen {
    Start(Start),
    Writing(Session),
}

/// The start line, and what the log says to offer on it.
struct Start {
    input: String,
    error: Option<String>,
    /// Taken by Enter on an empty line: the rest of an unfinished session,
    /// or else the last goal.
    suggested: Option<Goal>,
    /// When the unfinished session began, if `suggested` is its rest.
    unfinished_from: Option<NaiveTime>,
    /// "Yesterday: 523 words · 4 days in a row"
    record: Option<String>,
    /// The menu, while it's open over the start line.
    menu: Option<Menu>,
    /// The page chosen to write on; `None` for a new one.
    page: Option<Listed>,
}

impl Start {
    /// Chooses the page for the next session. The rest of an unfinished
    /// session belongs on its own page, so choosing another lets it go.
    fn choose(&mut self, page: Option<Listed>, logbook: &Option<Logbook>) {
        let path = |page: &Option<Listed>| page.as_ref().map(|page| page.path.clone());
        if path(&page) != path(&self.page) && self.unfinished_from.take().is_some() {
            self.suggested = logbook.as_ref().and_then(|log| logbook::last_goal(&log.entries()));
        }
        self.page = page;
    }
}

struct Session {
    page: Page,
    goal: Goal,
    started: Instant,
    /// The same moment on the clock, for the log.
    begun: DateTime<Local>,
    last_save: Instant,
    save_error: Option<String>,
    /// Stays true once the goal has been reached, even if words are deleted.
    reached: bool,
    /// Set when leaving early with the panic sentence.
    released: bool,
    /// What's been typed towards the panic sentence, while it's showing.
    panic: Option<String>,
    /// How far the page has been scrolled back, once the goal is reached.
    /// Zero is the end, with the typing line in the centre.
    scroll: f32,
    logbook: Option<Logbook>,
    /// Set once the session's line is in the log.
    logged: bool,
    /// Since when the terminal has been up, if it is.
    hidden_since: Option<Instant>,
    /// Time spent behind the terminal before that.
    hidden_for: Duration,
    /// The name being typed, after Ctrl+S on a page without one.
    naming: Option<Naming>,
    /// What was just saved, and since when it's been showing.
    note: Option<(String, Instant)>,
}

/// The line for giving a page its name.
#[derive(Default)]
struct Naming {
    name: String,
    /// Brought up by Esc on the way out: Esc again leaves without a name.
    leaving: bool,
    problem: Option<String>,
}

impl Session {
    fn new(page: Page, goal: Goal, logbook: Option<Logbook>) -> Self {
        let now = Instant::now();
        Self {
            page,
            goal,
            started: now,
            begun: Local::now(),
            last_save: now,
            save_error: None,
            reached: false,
            released: false,
            panic: None,
            scroll: 0.0,
            logbook,
            logged: false,
            hidden_since: None,
            hidden_for: Duration::ZERO,
            naming: None,
            note: None,
        }
    }

    /// Time spent on the session, leaving out the time behind the terminal.
    fn elapsed(&self) -> Duration {
        self.elapsed_at(Instant::now())
    }

    fn elapsed_at(&self, now: Instant) -> Duration {
        let hidden = self.hidden_for + self.hidden_since.map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        now.saturating_duration_since(self.started).saturating_sub(hidden)
    }

    fn set_hidden(&mut self, hidden: bool, now: Instant) {
        match (hidden, self.hidden_since) {
            (true, None) => self.hidden_since = Some(now),
            (false, Some(since)) => {
                self.hidden_for += now.saturating_duration_since(since);
                self.hidden_since = None;
            }
            _ => {}
        }
    }

    /// The session's line in the log, as it stands.
    fn entry(&self) -> Entry {
        let outcome = if self.reached {
            Outcome::Reached
        } else if self.released {
            Outcome::LeftEarly
        } else {
            Outcome::Unfinished
        };
        let time = self.begun.time();
        Entry {
            date: self.begun.date_naive(),
            start: NaiveTime::from_hms_opt(time.hour(), time.minute(), 0).unwrap_or(time),
            goal: self.goal,
            words: self.page.session_words(),
            minutes: self.elapsed().as_secs() / 60,
            outcome,
        }
    }

    /// Puts the session in the log when it ends properly. If it never does,
    /// the next start finds its running line instead.
    fn log(&mut self) {
        if let (Some(logbook), false) = (&self.logbook, self.logged) {
            // The log is a record, not the writing: a failure here mustn't
            // keep anyone on the page.
            let _ = logbook.finish(&self.entry());
            self.logged = true;
        }
    }

    fn progress(&self) -> f32 {
        if self.reached {
            1.0
        } else {
            self.goal.progress(self.page.session_words(), self.elapsed())
        }
    }

    fn locked(&self) -> bool {
        !self.reached && !self.released
    }

    fn check_goal(&mut self) {
        if !self.reached && self.progress() >= 1.0 {
            self.reached = true;
            self.save();
        }
    }

    /// Saves the page, and the session's running line in the log.
    fn save(&mut self) -> bool {
        self.last_save = Instant::now();
        if let (Some(logbook), false) = (&self.logbook, self.logged) {
            let _ = logbook.write_running(&self.entry(), self.page.path());
        }
        match self.page.save() {
            Ok(()) => {
                self.save_error = None;
                true
            }
            Err(e) => {
                self.save_error = Some(e.to_string());
                false
            }
        }
    }

    /// Gives the page the name being typed, unless the name won't do. True
    /// once it's named and saved.
    fn name(&mut self, pages: &Path) -> bool {
        let Some(naming) = &mut self.naming else { return false };
        let to = match page::named_file(pages, &naming.name) {
            Ok(to) => to,
            Err(problem) => {
                naming.problem = Some(problem.to_owned());
                return false;
            }
        };
        if to.exists() {
            naming.problem = Some(format!("There's already a page called {}. Try another name.", stem(&to)));
            return false;
        }
        if let Err(e) = self.page.rename(to.clone()) {
            naming.problem = Some(format!("Couldn't save it: {e}"));
            return false;
        }
        self.naming = None;
        self.note = Some((format!("Saved as {}", short_path(&to)), Instant::now()));
        // The running line follows the page to its name.
        self.save()
    }
}

impl WriterApp {
    /// `goal` comes from `--goal`; if it's valid, the session starts right away.
    pub fn new(
        settings: Settings,
        windowed: bool,
        pages: PathBuf,
        logbook: Option<Logbook>,
        goal: Option<String>,
    ) -> Self {
        // A session that didn't end properly goes into the log first, and if
        // it was today and short of its goal, the rest of it is offered, on
        // the page it was on.
        let leftover = logbook.as_ref().and_then(Logbook::recover);
        let entries = logbook.as_ref().map(Logbook::entries).unwrap_or_default();
        let today = Local::now().date_naive();
        let unfinished = leftover
            .filter(|(entry, _)| entry.outcome == Outcome::Unfinished && entry.date == today)
            .and_then(|(entry, page)| Some((entry.start, entry.remaining()?, page)));
        let mut start = Start {
            input: goal.unwrap_or_default(),
            error: None,
            suggested: unfinished.as_ref().map(|(_, rest, _)| *rest).or_else(|| logbook::last_goal(&entries)),
            unfinished_from: unfinished.as_ref().map(|(from, _, _)| *from),
            record: logbook::record(&entries, today),
            menu: None,
            // A page that never got a word isn't there; that goes on a new one.
            page: unfinished.and_then(|(_, _, page)| page).filter(|page| page.is_file()).map(Listed::of),
        };
        let screen = match begin(&pages, &logbook, &start.input, None) {
            Ok(Some(session)) => Screen::Writing(session),
            Ok(None) => Screen::Start(start),
            Err(error) => {
                start.error = Some(error);
                Screen::Start(start)
            }
        };
        Self {
            settings,
            windowed,
            pages,
            logbook,
            screen,
            level: WindowLevel::Normal,
            task_manager_in_front: lock::task_manager_in_front,
            monitors: lock::monitors,
            placed: lock::monitors(),
            terminal: Terminal::here(),
        }
    }

    /// Ctrl+Space: the terminal over everything, or back to what was there.
    fn switch_terminal(&mut self, ctx: &egui::Context) {
        self.terminal.switch();
        let shown = self.terminal.shown();
        if let Screen::Writing(session) = &mut self.screen {
            session.set_hidden(shown, Instant::now());
        }
        let title = if shown { terminal::TITLE } else { "Writer" };
        ctx.send_viewport_cmd(ViewportCommand::Title(title.to_owned()));
        ctx.request_repaint();
    }

    /// The blank windows on the other monitors during a locked session, one
    /// per monitor. They mirror the page's window level, so they step aside
    /// for Task Manager along with it.
    fn covers(&self) -> Vec<(egui::ViewportId, egui::ViewportBuilder)> {
        let Screen::Writing(session) = &self.screen else { return Vec::new() };
        // If monitors come or go mid-session, the numbering can shift, and a
        // cover must never land on the page: they're dropped instead.
        if self.windowed || !session.locked() || (self.monitors)() != self.placed {
            return Vec::new();
        }
        self.placed
            .others()
            .map(|index| {
                let title = if self.terminal.shown() { terminal::TITLE } else { "Writer" };
                let mut builder = egui::ViewportBuilder::default()
                    .with_title(title)
                    .with_monitor(index)
                    .with_decorations(false)
                    .with_taskbar(false)
                    .with_active(false)
                    .with_window_level(self.level);
                // Alt+Tab still lists the covers, so they carry the app's icon.
                if let Some(icon) = icon::icon() {
                    builder = builder.with_icon(icon);
                }
                (egui::ViewportId::from_hash_of(("cover", index)), builder)
            })
            .collect()
    }

    fn set_level(&mut self, ctx: &egui::Context, level: WindowLevel) {
        if self.level != level {
            self.level = level;
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(level));
        }
    }

    /// Keeps a locked session in front: refuses to close, stays on top and
    /// takes focus back, except from Task Manager.
    fn hold_lock(&mut self, ctx: &egui::Context, locked: bool) {
        if !locked {
            self.set_level(ctx, WindowLevel::Normal);
            return;
        }
        let (close_requested, focused, minimized) = ctx.input(|i| {
            let viewport = i.viewport();
            (viewport.close_requested(), viewport.focused, viewport.minimized)
        });
        if close_requested {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
        // A window that started without focus never hears that it lost it, so
        // "unknown" counts as not focused.
        let focused = focused == Some(true);
        if !focused && (self.task_manager_in_front)() {
            self.set_level(ctx, WindowLevel::Normal);
            return;
        }
        self.set_level(ctx, WindowLevel::AlwaysOnTop);
        if minimized == Some(true) {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        }
        if !focused || minimized == Some(true) {
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
    }

    fn start_screen(&mut self, ui: &egui::Ui, events: &[Event]) {
        let Screen::Start(start) = &mut self.screen else { return };
        if let Some(open) = &mut start.menu {
            match open.show(ui, events) {
                menu::Outcome::Open => {}
                menu::Outcome::Closed => {
                    start.menu = None;
                    ui.ctx().request_repaint();
                }
                menu::Outcome::WriteOn(page) => {
                    start.menu = None;
                    start.choose(Some(page), &self.logbook);
                    ui.ctx().request_repaint();
                }
            }
            return;
        }
        let mut session = None;
        for event in events {
            match event {
                Event::Text(text) => push_limited(&mut start.input, text, 12),
                Event::Key { key, pressed: true, .. } => match key {
                    Key::Tab => start.menu = Some(Menu::open(&self.pages)),
                    Key::Backspace => {
                        start.input.pop();
                    }
                    Key::Enter => {
                        let input = match (start.input.is_empty(), start.suggested) {
                            (true, Some(goal)) => goal.input(),
                            _ => start.input.clone(),
                        };
                        let page = start.page.as_ref().map(|page| page.path.as_path());
                        match begin(&self.pages, &self.logbook, &input, page) {
                            Ok(started) => session = started,
                            Err(e) => start.error = Some(e),
                        }
                    }
                    // A step back: from a chosen page to a new one, then out.
                    Key::Escape if start.page.is_some() => start.choose(None, &self.logbook),
                    Key::Escape => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
                    _ => {}
                },
                _ => {}
            }
        }
        if let Some(session) = session {
            self.screen = Screen::Writing(session);
            ui.ctx().request_repaint();
            return;
        }

        let painter = ui.painter();
        let rect = ui.max_rect();
        let font = FontId::monospace(TEXT_SIZE);
        let note = FontId::monospace(NOTE_SIZE);
        let (left, _) = column(painter, rect);

        // The menu's word sits where its heading will be once it's open.
        if menu::link(ui, pos2(left, rect.top() + 32.0), Align2::LEFT_TOP, "Menu", "start menu").clicked() {
            start.menu = Some(Menu::open(&self.pages));
            ui.ctx().request_repaint();
        }
        // With a page chosen, a way back to a blank one beside it.
        if start.page.is_some() {
            let x = left + painter.layout_no_wrap("Menu   ".to_owned(), note.clone(), FAINT).size().x;
            if menu::link(ui, pos2(x, rect.top() + 32.0), Align2::LEFT_TOP, "New page", "start new page").clicked() {
                start.choose(None, &self.logbook);
                ui.ctx().request_repaint();
            }
        }

        let line = painter.layout_no_wrap(format!("Goal: {}", start.input), font.clone(), INK);
        let pos = pos2(left, rect.center().y - line.size().y / 2.0);
        let caret = Rect::from_min_size(pos + vec2(line.size().x + 1.0, 0.0), vec2(2.0, line.size().y));
        painter.galley(pos, line, INK);
        painter.rect_filled(caret, 0.0, FAINT);

        // An empty line shows what Enter will take, faintly, after the caret.
        let suggested = start.suggested.filter(|_| start.input.is_empty());
        if let Some(goal) = suggested {
            painter.text(caret.right_top() + vec2(2.0, 0.0), Align2::LEFT_TOP, goal.input(), font, FAINT);
        }
        let hint = match (Goal::parse(&start.input), suggested, start.unfinished_from) {
            (Ok(None), Some(rest), Some(from)) => {
                format!("Unfinished from {}: {} to go. Enter to finish it.", from.format("%H:%M"), rest.describe())
            }
            (Ok(None), Some(goal), None) => {
                format!("{}, like last time. Enter to start, or type another goal.", goal.describe())
            }
            (Ok(None), None, _) if start.page.is_some() => "500 for words, 25m for minutes.".to_owned(),
            (Ok(None), None, _) => "500 for words, 25m for minutes. Esc to quit.".to_owned(),
            (Ok(Some(goal)), ..) => goal.describe(),
            (Err(problem), ..) => problem.to_owned(),
        };
        let below = caret.bottom() + 12.0;
        painter.text(pos2(left, below), Align2::LEFT_TOP, hint, note.clone(), FAINT);
        if let Some(error) = &start.error {
            painter.text(pos2(left, below + 24.0), Align2::LEFT_TOP, error, note.clone(), INK);
        }
        let bottom = rect.center_bottom() - vec2(0.0, 24.0);
        if let Some(record) = &start.record {
            painter.text(bottom - vec2(0.0, 24.0), Align2::CENTER_BOTTOM, record, note.clone(), FAINT);
        }
        let on = match &start.page {
            Some(page) => format!("Writing on {} ({}). Esc for a new page.", page.name, menu::words(page.words)),
            None => "A new page. Once you reach your goal, Ctrl+S gives it a name.".to_owned(),
        };
        painter.text(bottom, Align2::CENTER_BOTTOM, on, note, FAINT);
    }

    fn writing_screen(&mut self, ui: &egui::Ui, events: &[Event]) {
        let Screen::Writing(session) = &mut self.screen else { return };
        let mut leave = false;
        let mut name = false;
        // Reading back is only for after the goal: until then the page only
        // goes forward.
        let page_height = ui.max_rect().height() * 0.8;
        for event in events {
            // The name line takes the keys while it's up.
            if let Some(naming) = &mut session.naming {
                match event {
                    Event::Text(text) => {
                        let room = NAME_CHARS.saturating_sub(naming.name.chars().count());
                        naming.name.extend(text.chars().filter(|&c| page::allowed_in_name(c)).take(room));
                        naming.problem = None;
                    }
                    Event::Key { key: Key::Backspace, pressed: true, .. } => {
                        naming.name.pop();
                        naming.problem = None;
                    }
                    Event::Key { key: Key::Enter, pressed: true, .. } => name = true,
                    Event::Key { key: Key::Escape, pressed: true, .. } => {
                        leave = naming.leaving;
                        session.naming = None;
                    }
                    _ => {}
                }
                continue;
            }
            match event {
                Event::Text(text) => match &mut session.panic {
                    Some(typed) => push_limited(typed, text, PANIC_SENTENCE.len() + 20),
                    None => {
                        session.scroll = 0.0;
                        session.page.type_text(text);
                    }
                },
                Event::MouseWheel { unit, delta, .. } if session.reached => {
                    let step = match unit {
                        MouseWheelUnit::Point => 1.0,
                        MouseWheelUnit::Line => LINE_SCROLL,
                        MouseWheelUnit::Page => page_height,
                    };
                    session.scroll += delta.y * step;
                }
                Event::Key { key, pressed: true, modifiers, .. } => match (key, &mut session.panic) {
                    (Key::Enter, Some(typed)) => {
                        if typed == PANIC_SENTENCE {
                            session.released = true;
                            leave = true;
                        }
                    }
                    (Key::Enter, None) => {
                        session.scroll = 0.0;
                        session.page.new_line();
                    }
                    (Key::Backspace, Some(typed)) => {
                        typed.pop();
                    }
                    (Key::Backspace, None) if self.settings.backspace => {
                        session.scroll = 0.0;
                        session.page.backspace();
                    }
                    (Key::PageUp, None) if session.reached => session.scroll += page_height,
                    (Key::PageDown, None) => session.scroll -= page_height,
                    (Key::End, None) => session.scroll = 0.0,
                    // Saving is for once the goal is reached; until then the
                    // page saves itself and the screen stays empty.
                    (Key::S, None) if modifiers.command && session.reached => {
                        if page::is_unnamed(session.page.path()) {
                            session.naming = Some(Naming::default());
                        } else if session.save() {
                            session.note = Some((format!("Saved: {}", short_path(session.page.path())), Instant::now()));
                        }
                    }
                    (Key::Escape, Some(_)) => session.panic = None,
                    (Key::Escape, None) => {
                        if session.locked() {
                            if self.settings.panic_exit {
                                session.panic = Some(String::new());
                            }
                        } else if page::is_unnamed(session.page.path()) && !session.page.is_blank() {
                            // Writing with no name asks for one on the way out.
                            session.naming = Some(Naming { leaving: true, ..Naming::default() });
                        } else {
                            leave = true;
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        if name {
            let leaving = session.naming.as_ref().is_some_and(|naming| naming.leaving);
            leave |= session.name(&self.pages) && leaving;
        }
        session.check_goal();
        // Only leave once the page is safely saved.
        if leave && session.save() {
            session.log();
            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
        }

        let painter = ui.painter();
        let rect = ui.max_rect();
        let (left, width) = column(painter, rect);

        // The page rolls up so the line being typed stays at the centre.
        // Reading back lays out the whole day, so it can go all the way up;
        // the tail starts at a line break, so lines wrap the same either way.
        let reading = session.scroll > 0.0;
        let text = session.page.tail(if reading { usize::MAX } else { TAIL_BYTES }).replace('\r', "");
        let galley = painter.layout(text, FontId::monospace(TEXT_SIZE), INK, width + 0.5);
        let end = galley.pos_from_cursor(galley.end());
        // At most, the first line comes down to near the top of the screen.
        let furthest = (end.center().y - rect.height() / 2.0 + 48.0).max(0.0);
        session.scroll = session.scroll.clamp(0.0, furthest);
        let origin = pos2(left, rect.center().y - end.center().y + session.scroll);
        let caret = Rect::from_min_size(origin + end.min.to_vec2() + vec2(1.0, 0.0), vec2(2.0, end.height()));
        painter.galley(origin, galley, INK);
        if session.scroll == 0.0 {
            painter.rect_filled(caret, 0.0, FAINT);
        }

        // Progress: a thin line along the bottom edge.
        let filled = rect.width() * session.progress();
        let line = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 3.0), pos2(rect.left() + filled, rect.bottom()));
        painter.rect_filled(line, 0.0, INK);

        let note = FontId::monospace(NOTE_SIZE);
        let bottom = rect.center_bottom() - vec2(0.0, 24.0);
        if let Some(error) = &session.save_error {
            painter.text(bottom, Align2::CENTER_BOTTOM, format!("Not saved: {error}"), note, INK);
        } else if let Some(typed) = &session.panic {
            let prompt = "To leave early, type this and press Enter. Esc to keep writing.";
            painter.text(bottom - vec2(0.0, 48.0), Align2::CENTER_BOTTOM, prompt, note.clone(), FAINT);
            painter.text(bottom - vec2(0.0, 24.0), Align2::CENTER_BOTTOM, PANIC_SENTENCE, note.clone(), FAINT);
            painter.text(bottom, Align2::CENTER_BOTTOM, format!("{typed}_"), note, INK);
        } else if let Some(naming) = &session.naming {
            let line = painter.layout_no_wrap(format!("Save as: {}", naming.name), note.clone(), INK);
            let pos = pos2(left, bottom.y - 24.0 - line.size().y);
            let caret = Rect::from_min_size(pos + vec2(line.size().x + 1.0, 0.0), vec2(2.0, line.size().y));
            painter.galley(pos, line, INK);
            painter.rect_filled(caret, 0.0, FAINT);
            let unnamed = stem(session.page.path());
            let hint = match (&naming.problem, page::named_file(&self.pages, &naming.name), naming.leaving) {
                (Some(problem), ..) => problem.clone(),
                (None, Ok(to), false) => format!("Enter saves it as {}. Esc cancels.", short_path(&to)),
                (None, Ok(to), true) => format!("Enter saves it as {} and leaves. Esc leaves it as {unnamed}.", short_path(&to)),
                (None, Err(_), false) => "A name for the page. Esc cancels.".to_owned(),
                (None, Err(_), true) => format!("A name for the page, or Esc to leave it as {unnamed}."),
            };
            painter.text(pos2(left, bottom.y), Align2::LEFT_BOTTOM, hint, note, FAINT);
        } else if session.scroll > 0.0 {
            let reading = "Reading back. Type, or press End, to return to the end.";
            painter.text(bottom, Align2::CENTER_BOTTOM, reading, note, FAINT);
        } else if let Some((saved, _)) = session.note.as_ref().filter(|(_, at)| at.elapsed() < NOTE_FOR) {
            painter.text(bottom, Align2::CENTER_BOTTOM, saved, note, FAINT);
        } else if session.reached {
            let tally = tally(session.page.session_words(), session.elapsed());
            let done = if page::is_unnamed(session.page.path()) {
                format!("Goal reached: {tally}. Ctrl+S names it. Esc to leave, or keep writing.")
            } else {
                format!("Goal reached: {tally}. Esc to leave, or keep writing.")
            };
            painter.text(bottom, Align2::CENTER_BOTTOM, done, note, FAINT);
        }
    }
}

/// Starts a session if `input` is a valid goal: on `page` if one was chosen,
/// or else on a new one.
fn begin(pages: &Path, logbook: &Option<Logbook>, input: &str, page: Option<&Path>) -> Result<Option<Session>, String> {
    let Ok(Some(goal)) = Goal::parse(input) else { return Ok(None) };
    let path = page.map_or_else(|| page::new_file(pages, Local::now()), Path::to_owned);
    let page = Page::open(path).map_err(|e| format!("Couldn't open the page: {e}"))?;
    let mut session = Session::new(page, goal, logbook.clone());
    // Writes the running line, so even a session cut short at once is logged.
    session.save();
    Ok(Some(session))
}

/// What a session has done, for once the goal is reached: "523 words in 27
/// minutes". Numbers never show before that.
fn tally(words: usize, elapsed: Duration) -> String {
    let words = if words == 1 { "1 word".to_owned() } else { format!("{words} words") };
    match elapsed.as_secs() / 60 {
        0 => format!("{words} in under a minute"),
        1 => format!("{words} in 1 minute"),
        minutes => format!("{words} in {minutes} minutes"),
    }
}

/// A page's name: its file name without `.txt`.
fn stem(path: &Path) -> String {
    path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A page's file with just its folder, e.g. `Writing\Short story.txt`.
fn short_path(path: &Path) -> String {
    let file = path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
    match path.parent().and_then(Path::file_name) {
        Some(folder) => format!("{}\\{file}", folder.to_string_lossy()),
        None => file.into_owned(),
    }
}

/// Takes Ctrl+Space out of the events: true if it was pressed (held down,
/// it repeats, and those don't count).
fn take_switch(events: &mut Vec<Event>) -> bool {
    let mut pressed = false;
    events.retain(|event| match event {
        Event::Key { key: Key::Space, pressed: down, repeat, modifiers, .. } if modifiers.ctrl && !modifiers.alt => {
            pressed ^= *down && !*repeat;
            false
        }
        _ => true,
    });
    pressed
}

/// Adds typed text to a short input line, up to `max_chars` characters.
fn push_limited(line: &mut String, text: &str, max_chars: usize) {
    let room = max_chars.saturating_sub(line.chars().count());
    line.extend(text.chars().filter(|c| !c.is_control()).take(room));
}

/// The writing column: its left edge and width, centred on the page.
pub fn column(painter: &Painter, rect: Rect) -> (f32, f32) {
    let char_width = painter
        .layout_no_wrap("M".to_owned(), FontId::monospace(TEXT_SIZE), INK)
        .size()
        .x;
    let width = (char_width * COLUMN).min(rect.width() - 4.0 * char_width);
    (rect.center().x - width / 2.0, width)
}

impl eframe::App for WriterApp {
    /// Runs every tick, even while the window is hidden: the lock and autosave.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Screen::Writing(session) = &mut self.screen else { return };
        session.check_goal();
        let since = session.last_save.elapsed();
        if (session.page.is_dirty() && since >= AUTOSAVE_EVERY) || since >= LOG_EVERY {
            session.save();
        }
        let locked = session.locked();
        if !self.windowed {
            self.hold_lock(ctx, locked);
        }
        ctx.request_repaint_after(LOCK_TICK);
    }

    /// Typing and drawing. Keystrokes are only read here: `logic` sees stale
    /// events while the window is hidden.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.terminal.load_font(ui.ctx());
        let mut events = ui.input(|i| i.events.clone());
        if take_switch(&mut events) {
            self.switch_terminal(ui.ctx());
        }
        // Behind the terminal, the screens hear nothing: no typing, no Esc.
        if self.terminal.shown() {
            self.terminal.tick(Instant::now());
            self.terminal.show(ui);
        } else {
            // The pointer is for the start screen and its menu; writing has none.
            if matches!(self.screen, Screen::Writing(_)) {
                ui.ctx().set_cursor_icon(CursorIcon::None);
            }
            ui.painter().rect_filled(ui.max_rect(), 0.0, PAPER);
            match self.screen {
                Screen::Start(_) => self.start_screen(ui, &events),
                Screen::Writing(_) => self.writing_screen(ui, &events),
            }
        }
        // Covers live as long as they're shown each frame; once the session
        // is no longer locked, they close.
        for (nth, (id, builder)) in self.covers().into_iter().enumerate() {
            let monitor = builder.monitor.unwrap_or(nth);
            let terminal = &mut self.terminal;
            ui.ctx().show_viewport_immediate(id, builder, |ui, _| {
                if terminal.shown() {
                    terminal.show_cover(ui, monitor, nth);
                } else {
                    ui.ctx().set_cursor_icon(CursorIcon::None);
                    ui.painter().rect_filled(ui.max_rect(), 0.0, PAPER);
                }
                if ui.input(|i| i.viewport().close_requested()) {
                    ui.ctx().send_viewport_cmd(ViewportCommand::CancelClose);
                }
            });
        }
    }

    fn on_exit(&mut self) {
        if let Screen::Writing(session) = &mut self.screen {
            session.save();
            // A session closing while still locked (Windows shutting down,
            // say) is left running, so the next start offers the rest of it.
            if !session.locked() {
                session.log();
            }
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let colour = if self.terminal.shown() { terminal::CLEAR } else { PAPER };
        colour.to_normalized_gamma_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    const FIRST: &str = "The gallery opens at ten. Before that there is an hour where nothing happens, and that hour is the reason I keep coming back. The light moves across the floor in long pale bars, the radiators tick, and somewhere upstairs a door closes as if the whole building agreed with it.";
    const SECOND: &str = "Æblerne på bordet er ved at blive bløde. Jeg skriver dem ned alligevel, før de forsvinder.";
    const THIRD: &str = "And then the third paragraph, still going, with the line I am typing held in the middle of the page while everything above it rolls up and away.";

    /// One frame of a headless session with the given window state. Returns
    /// the window commands the app sent.
    fn frame(
        harness: &mut Harness<'_, WriterApp>,
        focused: Option<bool>,
        minimized: bool,
        close: bool,
        event: Option<Event>,
    ) -> Vec<ViewportCommand> {
        let info = harness.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default();
        info.focused = focused;
        info.minimized = Some(minimized);
        if close {
            info.events.push(egui::ViewportEvent::Close);
        }
        if let Some(event) = event {
            harness.event(event);
        }
        harness.step();
        harness.output().viewport_output[&egui::ViewportId::ROOT].commands.clone()
    }

    fn press(key: Key) -> Option<Event> {
        Some(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() })
    }

    /// A test's pages folder, emptied.
    fn scratch(name: &str) -> PathBuf {
        let pages = std::env::temp_dir().join(format!("writer-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pages);
        pages
    }

    /// The log for a test, kept inside its pages folder so it's cleaned up too.
    fn log_of(pages: &Path) -> Logbook {
        Logbook::new(pages.join("app"))
    }

    fn locked_session(name: &str, settings: Settings, goal: &str) -> (Harness<'static, WriterApp>, PathBuf) {
        let pages = scratch(name);
        let app_pages = pages.clone();
        let goal = goal.to_owned();
        let mut harness = Harness::builder()
            .build_eframe(move |_| WriterApp::new(settings, false, app_pages.clone(), Some(log_of(&app_pages)), Some(goal)));
        harness.state_mut().task_manager_in_front = || false;
        harness.state_mut().monitors = one_monitor;
        harness.state_mut().placed = one_monitor();
        (harness, pages)
    }

    fn one_monitor() -> Monitors {
        Monitors { count: 1, primary: 0 }
    }

    /// The primary monitor is listed last, as on a laptop with two screens.
    fn three_monitors() -> Monitors {
        Monitors { count: 3, primary: 2 }
    }

    /// The start screen, with the log as it is.
    fn start_screen(pages: &Path) -> Harness<'static, WriterApp> {
        let app_pages = pages.to_owned();
        let mut harness = Harness::builder()
            .build_eframe(move |_| WriterApp::new(Settings::default(), true, app_pages.clone(), Some(log_of(&app_pages)), None));
        harness.step();
        harness
    }

    fn entry(goal: Goal, words: usize, minutes: u64, outcome: Outcome) -> Entry {
        let start = NaiveTime::from_hms_opt(9, 14, 0).unwrap();
        Entry { date: Local::now().date_naive(), start, goal, words, minutes, outcome }
    }

    /// The one page in a test's folder, and what's on it.
    fn only_page(pages: &Path) -> (PathBuf, String) {
        let list = page::all_pages(pages);
        assert_eq!(list.len(), 1, "{list:?}");
        let path = list[0].path.clone();
        let text = std::fs::read_to_string(&path).unwrap();
        (path, text)
    }

    fn ctrl_s() -> Option<Event> {
        Some(Event::Key { key: Key::S, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND })
    }

    fn session<'a>(h: &'a Harness<'_, WriterApp>) -> &'a Session {
        match &h.state().screen {
            Screen::Writing(session) => session,
            Screen::Start(_) => panic!("no session"),
        }
    }

    #[test]
    fn lock_holds_until_the_goal_then_lets_go() {
        use ViewportCommand::{CancelClose, Close, Focus, Minimized};
        let started = Instant::now();
        let (mut h, pages) = locked_session("lock", Settings::default(), "2s");

        // In front and on top.
        let commands = frame(&mut h, Some(true), false, false, None);
        assert!(matches!(h.state().screen, Screen::Writing(_)), "--goal starts the session");
        assert_eq!(h.state().level, WindowLevel::AlwaysOnTop);
        assert!(!commands.contains(&Focus) && !commands.contains(&CancelClose));

        // Alt+F4 or the close button: refused.
        assert!(frame(&mut h, Some(true), false, true, None).contains(&CancelClose));

        // Switched away, or focus never arrived: take it back.
        assert!(frame(&mut h, Some(false), false, false, None).contains(&Focus));
        assert!(frame(&mut h, None, false, false, None).contains(&Focus));

        // Minimised: restore and take focus.
        let commands = frame(&mut h, Some(false), true, false, None);
        assert!(commands.contains(&Minimized(false)) && commands.contains(&Focus));

        // Task Manager in front: step aside and leave it alone.
        h.state_mut().task_manager_in_front = || true;
        let commands = frame(&mut h, Some(false), false, false, None);
        assert!(commands.contains(&ViewportCommand::WindowLevel(WindowLevel::Normal)));
        assert!(!commands.contains(&Focus));
        h.state_mut().task_manager_in_front = || false;
        let commands = frame(&mut h, Some(true), false, false, None);
        assert!(commands.contains(&ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop)));

        // Typing goes on the page; Esc does nothing while locked.
        frame(&mut h, Some(true), false, false, Some(Event::Text("Hej æøå @ é".to_owned())));
        assert!(!frame(&mut h, Some(true), false, false, press(Key::Escape)).contains(&Close));

        // Goal reached: no longer on top, closing allowed, Esc leaves. The
        // page has no name, so the first Esc asks for one, and the second
        // leaves it under its time.
        std::thread::sleep(Duration::from_millis(2300).saturating_sub(started.elapsed()));
        let commands = frame(&mut h, Some(true), false, false, None);
        assert!(commands.contains(&ViewportCommand::WindowLevel(WindowLevel::Normal)));
        assert!(!frame(&mut h, Some(false), false, true, None).iter().any(|c| *c == CancelClose || *c == Focus));
        assert!(!frame(&mut h, Some(true), false, false, press(Key::Escape)).contains(&Close));
        assert!(frame(&mut h, Some(true), false, false, press(Key::Escape)).contains(&Close));

        let (path, saved) = only_page(&pages);
        assert!(page::is_unnamed(&path), "{path:?}");
        assert_eq!(saved, "Hej æøå @ é");
        let logged = log_of(&pages).entries();
        assert_eq!(logged.len(), 1);
        assert_eq!((logged[0].goal, logged[0].words, logged[0].outcome), (Goal::Time(2), 4, Outcome::Reached));
        assert!(log_of(&pages).recover().is_none(), "leaving properly ends the running line");
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn panic_sentence_releases_the_lock() {
        let (mut h, pages) = locked_session("panic", Settings { panic_exit: true, ..Settings::default() }, "100");
        frame(&mut h, Some(true), false, false, Some(Event::Text("Some words".to_owned())));
        frame(&mut h, Some(true), false, false, press(Key::Escape));
        frame(&mut h, Some(true), false, false, Some(Event::Text("I choose to stop".to_owned())));
        assert!(!frame(&mut h, Some(true), false, false, press(Key::Enter)).contains(&ViewportCommand::Close));
        frame(&mut h, Some(true), false, false, Some(Event::Text(PANIC_SENTENCE["I choose to stop".len()..].to_owned())));
        assert!(frame(&mut h, Some(true), false, false, press(Key::Enter)).contains(&ViewportCommand::Close));
        assert!(!frame(&mut h, Some(true), false, true, None).contains(&ViewportCommand::CancelClose));
        assert_eq!(only_page(&pages).1, "Some words");
        let logged = log_of(&pages).entries();
        assert_eq!((logged.len(), logged[0].words, logged[0].outcome), (1, 2, Outcome::LeftEarly));
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn backspace_can_be_turned_off() {
        let settings = Settings { backspace: false, ..Settings::default() };
        let (mut h, pages) = locked_session("typewriter", settings, "100");
        frame(&mut h, Some(true), false, false, Some(Event::Text("Typed".to_owned())));
        frame(&mut h, Some(true), false, false, press(Key::Backspace));
        let Screen::Writing(session) = &h.state().screen else { panic!("no session") };
        assert_eq!(session.page.tail(100), "Typed");
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn an_unfinished_session_is_offered_again_on_its_page() {
        let pages = scratch("unfinished");
        std::fs::create_dir_all(&pages).unwrap();
        let story = pages.join("Short story.txt");
        std::fs::write(&story, "Once upon a time").unwrap();
        // Cut off at 312 of 500 words this morning: Task Manager, or a crash.
        let cut = entry(Goal::Words(500), 312, 9, Outcome::Unfinished);
        log_of(&pages).write_running(&cut, &story).unwrap();

        let mut h = start_screen(&pages);
        let Screen::Start(start) = &h.state().screen else { panic!("not the start screen") };
        assert_eq!(start.suggested, Some(Goal::Words(188)));
        assert_eq!(start.unfinished_from, Some(cut.start));
        assert_eq!(start.page.as_ref().map(|page| page.name.as_str()), Some("Short story"));
        assert_eq!(log_of(&pages).entries(), vec![cut.clone()], "the cut session is logged");

        h.key_press(Key::Enter);
        h.step();
        assert_eq!(session(&h).goal, Goal::Words(188), "Enter takes the offer");
        assert_eq!(session(&h).page.path(), story);

        // Choosing a new page instead lets the offer go.
        log_of(&pages).write_running(&cut, &story).unwrap();
        let mut h = start_screen(&pages);
        h.key_press(Key::Escape);
        h.step();
        let Screen::Start(start) = &h.state().screen else { panic!("Esc is a step back, not out") };
        assert_eq!((start.page.is_none(), start.unfinished_from), (true, None));
        assert_eq!(start.suggested, Some(Goal::Words(500)), "the goal it was cut from");
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn the_last_goal_is_suggested_and_typing_replaces_it() {
        let pages = scratch("suggest");
        let mut yesterday = entry(Goal::Time(25 * 60), 300, 25, Outcome::Reached);
        yesterday.date = yesterday.date.pred_opt().unwrap();
        log_of(&pages).finish(&yesterday).unwrap();
        // Left early today: logged, but nothing to offer.
        let left = entry(Goal::Words(500), 40, 3, Outcome::LeftEarly);
        log_of(&pages).write_running(&left, &pages.join("2026-11-03 09.14.txt")).unwrap();

        let mut h = start_screen(&pages);
        let Screen::Start(start) = &h.state().screen else { panic!("not the start screen") };
        assert_eq!((start.suggested, start.unfinished_from), (Some(Goal::Words(500)), None));
        assert_eq!(start.record.as_deref(), Some("Today so far: 40 words"));

        h.event(Event::Text("7".to_owned()));
        h.key_press(Key::Enter);
        h.step();
        let Screen::Writing(session) = &h.state().screen else { panic!("Enter starts") };
        assert_eq!(session.goal, Goal::Words(7));
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn reading_back_only_after_the_goal() {
        let wheel = |notches: f32| Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, notches),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: Default::default(),
        };
        let scroll = |h: &Harness<'_, WriterApp>| match &h.state().screen {
            Screen::Writing(session) => session.scroll,
            Screen::Start(_) => panic!("no session"),
        };
        let (mut h, pages) = locked_session("readback", Settings::default(), "500");
        // A few screens' worth, about 50 words a paragraph.
        for _ in 0..9 {
            frame(&mut h, Some(true), false, false, Some(Event::Text(FIRST.to_owned())));
            frame(&mut h, Some(true), false, false, press(Key::Enter));
        }
        frame(&mut h, Some(true), false, false, Some(wheel(5.0)));
        frame(&mut h, Some(true), false, false, press(Key::PageUp));
        assert_eq!(scroll(&h), 0.0, "no looking back before the goal");

        frame(&mut h, Some(true), false, false, Some(Event::Text(FIRST.to_owned())));
        frame(&mut h, Some(true), false, false, Some(wheel(5.0)));
        assert_eq!(scroll(&h), 5.0 * LINE_SCROLL);
        frame(&mut h, Some(true), false, false, press(Key::PageUp));
        frame(&mut h, Some(true), false, false, press(Key::PageUp));
        let furthest = scroll(&h);
        frame(&mut h, Some(true), false, false, press(Key::PageUp));
        assert_eq!(scroll(&h), furthest, "it stops at the top of the page");
        frame(&mut h, Some(true), false, false, press(Key::PageDown));
        assert!(scroll(&h) < furthest);

        // Typing goes back to the end, and on the end of the page.
        frame(&mut h, Some(true), false, false, Some(Event::Text(" More.".to_owned())));
        assert_eq!(scroll(&h), 0.0);
        frame(&mut h, Some(true), false, false, Some(wheel(2.0)));
        frame(&mut h, Some(true), false, false, press(Key::End));
        assert_eq!(scroll(&h), 0.0);
        let Screen::Writing(session) = &h.state().screen else { panic!("no session") };
        assert!(session.page.tail(100).ends_with("as if the whole building agreed with it. More."));
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn covers_on_the_other_monitors_follow_the_lock() {
        let monitors = |h: &Harness<'_, WriterApp>| -> Vec<(Option<usize>, Option<WindowLevel>)> {
            let covers = h.state().covers();
            covers.iter().map(|(_, b)| (b.monitor, b.window_level)).collect()
        };
        let (mut h, pages) = locked_session("covers", Settings::default(), "2s");
        h.state_mut().monitors = three_monitors;
        h.state_mut().placed = three_monitors();

        frame(&mut h, Some(true), false, false, None);
        let on_top = Some(WindowLevel::AlwaysOnTop);
        assert_eq!(monitors(&h), vec![(Some(0), on_top), (Some(1), on_top)], "all but the page's");
        let (_, builder) = &h.state().covers()[0];
        assert_eq!((builder.decorations, builder.taskbar, builder.active), (Some(false), Some(false), Some(false)));

        // Task Manager in front: the covers step aside with the page.
        h.state_mut().task_manager_in_front = || true;
        frame(&mut h, Some(false), false, false, None);
        let normal = Some(WindowLevel::Normal);
        assert_eq!(monitors(&h), vec![(Some(0), normal), (Some(1), normal)]);
        h.state_mut().task_manager_in_front = || false;
        frame(&mut h, Some(true), false, false, None);

        // A monitor unplugged mid-session: the numbering may have shifted, so
        // no covers rather than one over the page.
        h.state_mut().monitors = || Monitors { count: 2, primary: 1 };
        assert!(h.state().covers().is_empty());
        h.state_mut().monitors = three_monitors;
        assert_eq!(h.state().covers().len(), 2);

        // Goal reached: the covers go.
        std::thread::sleep(Duration::from_millis(2100));
        frame(&mut h, Some(true), false, false, None);
        assert!(h.state().covers().is_empty());
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn no_covers_in_a_window_or_on_one_monitor() {
        let pages = scratch("nocovers");
        let mut h = start_screen(&pages);
        h.state_mut().monitors = three_monitors;
        h.state_mut().placed = three_monitors();
        assert!(h.state().covers().is_empty(), "not on the start screen");
        h.event(Event::Text("100".to_owned()));
        h.key_press(Key::Enter);
        h.step();
        assert!(matches!(h.state().screen, Screen::Writing(_)));
        assert!(h.state().covers().is_empty(), "not with --windowed");

        let (h, pages2) = locked_session("onemonitor", Settings::default(), "100");
        assert!(h.state().covers().is_empty());
        let _ = std::fs::remove_dir_all(&pages);
        let _ = std::fs::remove_dir_all(&pages2);
    }

    /// A pages folder with two days' pages from before pages had names, and a
    /// copy saved back then, written to last.
    fn written_before(name: &str) -> PathBuf {
        let pages = scratch(name);
        std::fs::create_dir_all(pages.join("Saved")).unwrap();
        let write = |path: PathBuf, text: &str, days_ago: u64| {
            std::fs::write(&path, text).unwrap();
            let when = std::time::SystemTime::now() - Duration::from_secs(days_ago * 86_400);
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(when).unwrap();
        };
        write(pages.join("2026-09-01.txt"), "An old page,\nfrom September.", 60);
        write(pages.join("2026-10-20.txt"), "A newer one.", 10);
        write(pages.join("Saved").join("Short story.txt"), "Once upon a time", 1);
        pages
    }

    fn menu<'a>(h: &'a Harness<'_, WriterApp>) -> Option<&'a Menu> {
        match &h.state().screen {
            Screen::Start(start) => start.menu.as_ref(),
            Screen::Writing(_) => None,
        }
    }

    #[test]
    fn the_menu_lists_and_reads_earlier_pages() {
        let pages = written_before("menu");
        let mut h = start_screen(&pages);
        h.event(Event::Text("50".to_owned()));
        h.run_steps(2);

        // Click "Menu" in the corner.
        h.get_by_label("Menu").click();
        h.run_steps(2);
        let rows = menu(&h).expect("the menu is open").rows();
        assert_eq!(rows.len(), 4);
        assert!(rows[0].starts_with("Short story ") && rows[0].ends_with("Yesterday"), "last written first: {rows:?}");
        assert!(rows[1].starts_with("2026-10-20 "));
        assert!(rows[2].starts_with("2026-09-01 "));
        assert_eq!(rows[3], format!("Open the {} folder", pages.file_name().unwrap().to_string_lossy()));

        // Down, Enter: the oldest page, read-only.
        h.key_press(Key::ArrowDown);
        h.key_press(Key::ArrowDown);
        h.key_press(Key::Enter);
        h.run_steps(2);
        assert_eq!(menu(&h).unwrap().reading(), Some("An old page,\nfrom September."));
        h.event(Event::Text("typing does nothing".to_owned()));
        h.key_press(Key::Backspace);
        h.run_steps(2);
        assert_eq!(menu(&h).unwrap().reading(), Some("An old page,\nfrom September."));
        assert_eq!(std::fs::read_to_string(pages.join("2026-09-01.txt")).unwrap(), "An old page,\nfrom September.");

        // Esc back to the list, still on that line; Esc again to the start line.
        h.key_press(Key::Escape);
        h.run_steps(2);
        assert_eq!(menu(&h).unwrap().reading(), None);
        assert_eq!(menu(&h).unwrap().selected(), 2);
        h.key_press(Key::Escape);
        h.run_steps(2);
        let Screen::Start(start) = &h.state().screen else { panic!("not the start screen") };
        assert!(start.menu.is_none() && start.page.is_none());
        assert_eq!(start.input, "50", "the goal typed before is still there");

        // Tab opens it too; clicking a line opens it.
        h.key_press(Key::Tab);
        h.run_steps(2);
        let first = menu(&h).unwrap().rows()[0].clone();
        h.get_by_label(&first).click();
        h.run_steps(2);
        assert_eq!(menu(&h).unwrap().reading(), Some("Once upon a time"));
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn an_earlier_page_is_written_on_from_the_menu() {
        let pages = written_before("write-on");
        let story = pages.join("Saved").join("Short story.txt");
        let mut h = start_screen(&pages);
        let chosen = |h: &Harness<'_, WriterApp>| match &h.state().screen {
            Screen::Start(start) if start.menu.is_none() => start.page.as_ref().map(|page| page.path.clone()),
            _ => panic!("not on the start line"),
        };

        // Read it, then "Write on it": back on the start line, with it chosen.
        h.key_press(Key::Tab);
        h.key_press(Key::Enter);
        h.run_steps(2);
        h.get_by_label("Write on it").click();
        h.run_steps(2);
        assert_eq!(chosen(&h), Some(story.clone()));

        // "New page" goes back to a blank one; Enter while reading chooses it again.
        h.get_by_label("New page").click();
        h.run_steps(2);
        assert_eq!(chosen(&h), None);
        h.key_press(Key::Tab);
        h.key_press(Key::Enter);
        h.key_press(Key::Enter);
        h.run_steps(2);
        assert_eq!(chosen(&h), Some(story.clone()));

        // The session goes on at the end, on a new paragraph, and only the
        // new words count. It has a name, so Ctrl+S just saves.
        h.event(Event::Text("3".to_owned()));
        h.key_press(Key::Enter);
        h.event(Event::Text("And then three".to_owned()));
        h.run_steps(2);
        assert!(session(&h).reached);
        h.event(ctrl_s().unwrap());
        h.run_steps(2);
        assert!(session(&h).naming.is_none());
        assert_eq!(std::fs::read_to_string(&story).unwrap(), "Once upon a time\n\nAnd then three");
        assert_eq!(page::all_pages(&pages).len(), 3, "no new page");
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn ctrl_s_names_a_new_page_once_the_goal_is_reached() {
        let pages = scratch("naming");
        std::fs::create_dir_all(&pages).unwrap();
        std::fs::write(pages.join("Taken.txt"), "Someone else's").unwrap();
        let mut h = start_screen(&pages);
        h.event(Event::Text("3".to_owned()));
        h.key_press(Key::Enter);
        h.run_steps(2);
        let unnamed = session(&h).page.path().to_owned();
        assert!(page::is_unnamed(&unnamed), "{unnamed:?}");

        // Before the goal, Ctrl+S does nothing.
        h.event(Event::Text("One two".to_owned()));
        h.event(ctrl_s().unwrap());
        h.run_steps(2);
        assert!(session(&h).naming.is_none());

        h.event(Event::Text(" three".to_owned()));
        h.event(ctrl_s().unwrap());
        h.event(Event::Text("Short/story?".to_owned()));
        h.run_steps(2);
        let naming = session(&h).naming.as_ref().expect("asks for a name");
        assert_eq!(naming.name, "Shortstory", "no / or ? in a file name");

        // A name that's taken is refused, and that page left alone.
        for _ in 0.."Shortstory".len() {
            h.key_press(Key::Backspace);
        }
        h.event(Event::Text("taken".to_owned()));
        h.key_press(Key::Enter);
        h.run_steps(2);
        let problem = session(&h).naming.as_ref().and_then(|naming| naming.problem.clone());
        assert!(problem.unwrap().contains("already a page called taken"));
        assert_eq!(std::fs::read_to_string(pages.join("Taken.txt")).unwrap(), "Someone else's");

        // Esc cancels; typing goes on the page again.
        h.key_press(Key::Escape);
        h.event(Event::Text(" four".to_owned()));
        h.run_steps(2);
        assert!(session(&h).naming.is_none());

        // Named: the file moves, and what's written after goes into it.
        h.event(ctrl_s().unwrap());
        h.event(Event::Text("Short story".to_owned()));
        h.key_press(Key::Enter);
        h.run_steps(2);
        let story = pages.join("Short story.txt");
        assert_eq!(session(&h).page.path(), story);
        assert!(!unnamed.exists());
        h.event(Event::Text(" five".to_owned()));
        h.event(ctrl_s().unwrap());
        h.run_steps(2);
        assert!(session(&h).naming.is_none(), "named pages just save");
        assert_eq!(std::fs::read_to_string(&story).unwrap(), "One two three four five");
        let (_, running) = log_of(&pages).recover().expect("still running");
        assert_eq!(running, Some(story), "the running line follows the page");
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn leaving_a_page_without_a_name_asks_for_one() {
        let pages = scratch("leave-naming");
        let mut h = start_screen(&pages);
        h.event(Event::Text("1".to_owned()));
        h.key_press(Key::Enter);
        h.event(Event::Text("Note".to_owned()));
        h.run_steps(2);
        assert!(!frame(&mut h, Some(true), false, false, press(Key::Escape)).contains(&ViewportCommand::Close));
        assert!(session(&h).naming.as_ref().is_some_and(|naming| naming.leaving));
        frame(&mut h, Some(true), false, false, Some(Event::Text("Shopping".to_owned())));
        assert!(frame(&mut h, Some(true), false, false, press(Key::Enter)).contains(&ViewportCommand::Close));
        assert_eq!(only_page(&pages), (pages.join("Shopping.txt"), "Note".to_owned()));

        // Nothing written: nothing to name, Esc just leaves.
        let pages2 = scratch("leave-blank");
        let mut h = start_screen(&pages2);
        h.event(Event::Text("1s".to_owned()));
        h.key_press(Key::Enter);
        h.run_steps(2);
        std::thread::sleep(Duration::from_millis(1100));
        frame(&mut h, Some(true), false, false, None);
        assert!(frame(&mut h, Some(true), false, false, press(Key::Escape)).contains(&ViewportCommand::Close));
        assert!(page::all_pages(&pages2).is_empty());
        let _ = std::fs::remove_dir_all(&pages);
        let _ = std::fs::remove_dir_all(&pages2);
    }

    #[test]
    fn the_folder_opens_from_the_menu() {
        static OPENED: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);
        let pages = written_before("folder");
        let mut h = start_screen(&pages);
        h.key_press(Key::Tab);
        h.run_steps(2);
        if let Screen::Start(Start { menu: Some(menu), .. }) = &mut h.state_mut().screen {
            menu.open_folder = |dir| *OPENED.lock().unwrap() = Some(dir.to_owned());
        }
        h.key_press(Key::End);
        h.key_press(Key::Enter);
        h.run_steps(2);
        assert_eq!(OPENED.lock().unwrap().as_deref(), Some(pages.as_path()));
        let _ = std::fs::remove_dir_all(&pages);
    }

    fn ctrl_space(pressed: bool) -> Option<Event> {
        Some(Event::Key { key: Key::Space, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::CTRL })
    }

    /// Ctrl+Space pressed and let go. Returns the commands from the press.
    fn switch(harness: &mut Harness<'_, WriterApp>) -> Vec<ViewportCommand> {
        let commands = frame(harness, Some(true), false, false, ctrl_space(true));
        frame(harness, Some(true), false, false, ctrl_space(false));
        commands
    }

    #[test]
    fn ctrl_space_puts_the_terminal_over_the_start_screen() {
        let pages = scratch("terminal-start");
        let mut h = start_screen(&pages);
        h.event(Event::Text("50".to_owned()));
        h.step();
        let commands = switch(&mut h);
        assert!(h.state().terminal.shown());
        assert!(commands.contains(&ViewportCommand::Title("Ubuntu".to_owned())));

        // Nothing gets through: no typing, no menu, and Esc doesn't quit.
        for key in [Key::Backspace, Key::Tab, Key::Enter, Key::Escape] {
            frame(&mut h, Some(true), false, false, Some(Event::Text("99".to_owned())));
            assert!(!frame(&mut h, Some(true), false, false, press(key)).contains(&ViewportCommand::Close));
        }

        // Held down, the key repeats; it switches once all the same.
        let commands = frame(&mut h, Some(true), false, false, ctrl_space(true));
        for _ in 0..3 {
            frame(&mut h, Some(true), false, false, ctrl_space(true));
        }
        frame(&mut h, Some(true), false, false, ctrl_space(false));
        assert!(!h.state().terminal.shown());
        assert!(commands.contains(&ViewportCommand::Title("Writer".to_owned())));
        let Screen::Start(start) = &h.state().screen else { panic!("not the start screen") };
        assert_eq!((start.input.as_str(), start.menu.is_none()), ("50", true), "as it was left");
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn the_lock_holds_behind_the_terminal() {
        let (mut h, pages) = locked_session("terminal-lock", Settings::default(), "100");
        h.state_mut().monitors = three_monitors;
        h.state_mut().placed = three_monitors();
        frame(&mut h, Some(true), false, false, Some(Event::Text("Before".to_owned())));
        switch(&mut h);

        // Still locked: refusing to close, taking focus back, on top.
        assert!(frame(&mut h, Some(true), false, true, None).contains(&ViewportCommand::CancelClose));
        assert!(frame(&mut h, Some(false), false, false, None).contains(&ViewportCommand::Focus));
        assert_eq!(h.state().level, WindowLevel::AlwaysOnTop);
        // The covers stay up, as terminals: htop and the log.
        let covers = h.state().covers();
        assert_eq!(covers.len(), 2);
        assert!(covers.iter().all(|(_, builder)| builder.title.as_deref() == Some("Ubuntu")));

        // Typing goes nowhere, and the session's clock stands still.
        frame(&mut h, Some(true), false, false, Some(Event::Text(" hidden".to_owned())));
        frame(&mut h, Some(true), false, false, press(Key::Escape));
        let Screen::Writing(session) = &h.state().screen else { panic!("no session") };
        assert!(session.hidden_since.is_some());

        switch(&mut h);
        frame(&mut h, Some(true), false, false, Some(Event::Text(" after".to_owned())));
        let Screen::Writing(session) = &h.state().screen else { panic!("no session") };
        assert_eq!(session.page.tail(100), "Before after");
        assert!(session.hidden_since.is_none() && session.hidden_for > Duration::ZERO);
        assert!(h.state().covers().iter().all(|(_, builder)| builder.title.as_deref() == Some("Writer")));
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn time_behind_the_terminal_does_not_count() {
        let pages = scratch("terminal-time");
        let page = Page::open(page::new_file(&pages, Local::now())).unwrap();
        let mut session = Session::new(page, Goal::Time(25 * 60), None);
        let start = session.started;
        let at = |seconds| start + Duration::from_secs(seconds);
        session.set_hidden(true, at(10));
        assert_eq!(session.elapsed_at(at(40)), Duration::from_secs(10), "stopped while hidden");
        session.set_hidden(false, at(70));
        session.set_hidden(false, at(80));
        assert_eq!(session.elapsed_at(at(100)), Duration::from_secs(40));
        session.set_hidden(true, at(100));
        session.set_hidden(true, at(110));
        session.set_hidden(false, at(120));
        assert_eq!(session.elapsed_at(at(130)), Duration::from_secs(50));
    }

    #[test]
    fn tally_counts_words_and_whole_minutes() {
        assert_eq!(tally(1, Duration::from_secs(59)), "1 word in under a minute");
        assert_eq!(tally(0, Duration::from_secs(60)), "0 words in 1 minute");
        assert_eq!(tally(523, Duration::from_secs(27 * 60 + 59)), "523 words in 27 minutes");
    }

    #[test]
    fn short_lines_are_limited_by_characters_not_bytes() {
        let mut line = "a".to_owned();
        push_limited(&mut line, "æøåæøåæøåæøå", 12);
        assert_eq!(line, "aæøåæøåæøåæø");
        push_limited(&mut line, "more", 12);
        assert_eq!(line.chars().count(), 12);
    }

    /// Renders the screens to `target/screenshots` for a look without running
    /// the locked app. Needs a GPU, so it only runs when asked:
    /// `cargo test -- --ignored screenshots`
    #[test]
    #[ignore]
    fn screenshots() {
        let pages = std::env::temp_dir().join(format!("writer-screens-{}", std::process::id()));
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("screenshots");
        std::fs::create_dir_all(&out).unwrap();

        let _ = std::fs::remove_dir_all(&pages);
        let app = |settings: Settings| {
            let pages = pages.clone();
            Harness::builder().with_size([1280.0, 800.0]).wgpu().build_eframe(move |cc| {
                crate::load_typewriter_font(&cc.egui_ctx);
                WriterApp::new(settings, true, pages.clone(), Some(log_of(&pages)), None)
            })
        };
        // Yesterday's session, so the record has a run of days to show.
        let mut yesterday = entry(Goal::Words(500), 512, 31, Outcome::Reached);
        yesterday.date = yesterday.date.pred_opt().unwrap();
        log_of(&pages).finish(&yesterday).unwrap();

        let mut harness = app(Settings::default());
        let shot = |harness: &mut Harness<'_, WriterApp>, name: &str| {
            harness.run_steps(2);
            harness.render().unwrap().save(out.join(format!("{name}.png"))).unwrap();
        };
        let typed = |harness: &Harness<'_, WriterApp>, text: &str| {
            for paragraph in text.split('\n') {
                if !paragraph.is_empty() {
                    harness.event(Event::Text(paragraph.to_owned()));
                }
                harness.key_press(Key::Enter);
            }
        };

        harness.event(Event::Text("120".to_owned()));
        shot(&mut harness, "1-start");

        harness.key_press(Key::Enter);
        harness.event(Event::Text(FIRST.to_owned()));
        shot(&mut harness, "2-writing");

        typed(&harness, &format!("\n{SECOND}\n\n{THIRD} {FIRST}"));
        harness.event(Event::Text(SECOND.to_owned()));
        harness.key_press(Key::Enter);
        shot(&mut harness, "3-goal-reached");

        harness.event(Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, 4.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: Default::default(),
        });
        shot(&mut harness, "4-reading-back");

        // Named once it's done.
        harness.key_press(Key::End);
        harness.event(ctrl_s().unwrap());
        harness.event(Event::Text("Short story".to_owned()));
        shot(&mut harness, "5-naming");
        harness.key_press(Key::Enter);
        shot(&mut harness, "6-named");
        drop(harness);

        // Cut off part way, with nothing to end it properly.
        let mut harness = app(Settings { panic_exit: true, ..Settings::default() });
        harness.run_steps(2);
        harness.event(Event::Text("1000".to_owned()));
        harness.key_press(Key::Enter);
        harness.event(Event::Text(THIRD.to_owned()));
        harness.key_press(Key::Escape);
        harness.event(Event::Text("I choose to stop".to_owned()));
        shot(&mut harness, "7-panic-exit");
        drop(harness);

        // The next start offers the rest of it...
        let mut harness = app(Settings::default());
        shot(&mut harness, "8-start-unfinished");
        drop(harness);

        // ...and after that, suggests the goal again.
        let mut harness = app(Settings::default());
        shot(&mut harness, "9-start-again");

        // The menu, with a few older pages to list.
        for (name, days_ago, text) in [("2026-10-29", 5, THIRD), ("Brev til Åse", 12, SECOND), ("2026-09-24", 40, FIRST)] {
            let path = pages.join(format!("{name}.txt"));
            std::fs::write(&path, text).unwrap();
            let when = std::time::SystemTime::now() - Duration::from_secs(days_ago * 86_400);
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(when).unwrap();
        }
        harness.key_press(Key::Tab);
        shot(&mut harness, "10-menu");
        harness.key_press(Key::Enter);
        harness.event(Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, -2.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: Default::default(),
        });
        shot(&mut harness, "11-reading");
        harness.key_press(Key::Enter);
        shot(&mut harness, "12-start-on-a-page");

        let _ = std::fs::remove_dir_all(&pages);
    }
}
