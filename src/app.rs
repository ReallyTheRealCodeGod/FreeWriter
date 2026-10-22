use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Event, FontId, Key, Painter, Rect, ViewportCommand,
    WindowLevel, pos2, vec2,
};

use crate::goal::Goal;
use crate::lock;
use crate::page::{self, Page};
use crate::settings::Settings;

const PAPER: Color32 = Color32::WHITE;
const INK: Color32 = Color32::from_rgb(0x1a, 0x1a, 0x1a);
const FAINT: Color32 = Color32::from_rgb(0x9a, 0x9a, 0x9a);

const TEXT_SIZE: f32 = 20.0;
const NOTE_SIZE: f32 = 14.0;
/// Width of the column in characters, like a typewriter's margins.
const COLUMN: f32 = 65.0;
/// How much of the end of the page to lay out; plenty to fill the screen.
const TAIL_BYTES: usize = 20_000;

const AUTOSAVE_EVERY: Duration = Duration::from_secs(3);
/// How often a locked session checks that it's still in front. Taking focus
/// back fakes an Alt key press, so this shouldn't be too frequent.
const LOCK_TICK: Duration = Duration::from_millis(250);

/// Typing this exactly ends a session early, if `panic_exit` is on.
const PANIC_SENTENCE: &str = "I choose to stop writing now, before I have reached my goal.";

pub struct WriterApp {
    settings: Settings,
    /// Development mode: a normal window and no lock.
    windowed: bool,
    /// The folder the day's pages go in.
    pages: PathBuf,
    screen: Screen,
    /// The window level last asked for, so it's only sent when it changes.
    level: WindowLevel,
    /// Whether Task Manager is in front. A field so tests can stand in for it.
    task_manager_in_front: fn() -> bool,
}

enum Screen {
    Start { input: String, error: Option<String> },
    Writing(Session),
}

struct Session {
    page: Page,
    goal: Goal,
    started: Instant,
    last_save: Instant,
    save_error: Option<String>,
    /// Stays true once the goal has been reached, even if words are deleted.
    reached: bool,
    /// Set when leaving early with the panic sentence.
    released: bool,
    /// What's been typed towards the panic sentence, while it's showing.
    panic: Option<String>,
}

impl Session {
    fn new(page: Page, goal: Goal) -> Self {
        let now = Instant::now();
        Self {
            page,
            goal,
            started: now,
            last_save: now,
            save_error: None,
            reached: false,
            released: false,
            panic: None,
        }
    }

    fn progress(&self) -> f32 {
        if self.reached {
            1.0
        } else {
            self.goal.progress(self.page.session_words(), self.started.elapsed())
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

    fn save(&mut self) -> bool {
        self.last_save = Instant::now();
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
}

impl WriterApp {
    /// `goal` comes from `--goal`; if it's valid, the session starts right away.
    pub fn new(settings: Settings, windowed: bool, pages: PathBuf, goal: Option<String>) -> Self {
        let input = goal.unwrap_or_default();
        let screen = match begin(&pages, &input) {
            Ok(Some(session)) => Screen::Writing(session),
            Ok(None) => Screen::Start { input, error: None },
            Err(error) => Screen::Start { input, error: Some(error) },
        };
        Self {
            settings,
            windowed,
            pages,
            screen,
            level: WindowLevel::Normal,
            task_manager_in_front: lock::task_manager_in_front,
        }
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
        let Screen::Start { input, error } = &mut self.screen else { return };
        let mut start = None;
        for event in events {
            match event {
                Event::Text(text) => push_limited(input, text, 12),
                Event::Key { key, pressed: true, .. } => match key {
                    Key::Backspace => {
                        input.pop();
                    }
                    Key::Enter => match begin(&self.pages, input) {
                        Ok(session) => start = session,
                        Err(e) => *error = Some(e),
                    },
                    Key::Escape => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
                    _ => {}
                },
                _ => {}
            }
        }
        if let Some(session) = start {
            self.screen = Screen::Writing(session);
            ui.ctx().request_repaint();
            return;
        }

        let painter = ui.painter();
        let rect = ui.max_rect();
        let font = FontId::monospace(TEXT_SIZE);
        let note = FontId::monospace(NOTE_SIZE);
        let (left, _) = column(painter, rect);

        let line = painter.layout_no_wrap(format!("Goal: {input}"), font, INK);
        let pos = pos2(left, rect.center().y - line.size().y / 2.0);
        let caret = Rect::from_min_size(pos + vec2(line.size().x + 1.0, 0.0), vec2(2.0, line.size().y));
        painter.galley(pos, line, INK);
        painter.rect_filled(caret, 0.0, FAINT);

        let hint = match Goal::parse(input) {
            Ok(Some(goal)) => goal.describe(),
            Ok(None) => "500 for words, 25m for minutes. Esc to quit.".to_owned(),
            Err(problem) => problem.to_owned(),
        };
        let below = caret.bottom() + 12.0;
        painter.text(pos2(left, below), Align2::LEFT_TOP, hint, note.clone(), FAINT);
        if let Some(error) = error {
            painter.text(pos2(left, below + 24.0), Align2::LEFT_TOP, error, note.clone(), INK);
        }
        let where_ = format!("Today's page: {}", page::today_file(&self.pages).display());
        painter.text(rect.center_bottom() - vec2(0.0, 24.0), Align2::CENTER_BOTTOM, where_, note, FAINT);
    }

    fn writing_screen(&mut self, ui: &egui::Ui, events: &[Event]) {
        let Screen::Writing(session) = &mut self.screen else { return };
        let mut leave = false;
        for event in events {
            match event {
                Event::Text(text) => match &mut session.panic {
                    Some(typed) => push_limited(typed, text, PANIC_SENTENCE.len() + 20),
                    None => session.page.type_text(text),
                },
                Event::Key { key, pressed: true, .. } => match (key, &mut session.panic) {
                    (Key::Enter, Some(typed)) => {
                        if typed == PANIC_SENTENCE {
                            session.released = true;
                            leave = true;
                        }
                    }
                    (Key::Enter, None) => session.page.new_line(),
                    (Key::Backspace, Some(typed)) => {
                        typed.pop();
                    }
                    (Key::Backspace, None) => session.page.backspace(),
                    (Key::Escape, Some(_)) => session.panic = None,
                    (Key::Escape, None) => {
                        if !session.locked() {
                            leave = true;
                        } else if self.settings.panic_exit {
                            session.panic = Some(String::new());
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        session.check_goal();
        // Only leave once the page is safely saved.
        if leave && session.save() {
            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
        }

        let painter = ui.painter();
        let rect = ui.max_rect();
        let (left, width) = column(painter, rect);

        // The page rolls up so the line being typed stays at the centre.
        let text = session.page.tail(TAIL_BYTES).replace('\r', "");
        let galley = painter.layout(text, FontId::monospace(TEXT_SIZE), INK, width + 0.5);
        let end = galley.pos_from_cursor(galley.end());
        let origin = pos2(left, rect.center().y - end.center().y);
        let caret = Rect::from_min_size(origin + end.min.to_vec2() + vec2(1.0, 0.0), vec2(2.0, end.height()));
        painter.galley(origin, galley, INK);
        painter.rect_filled(caret, 0.0, FAINT);

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
        } else if session.reached {
            let done = "Goal reached. Esc to leave, or keep writing.";
            painter.text(bottom, Align2::CENTER_BOTTOM, done, note, FAINT);
        }
    }
}

/// Starts a session on today's page if `input` is a valid goal.
fn begin(pages: &Path, input: &str) -> Result<Option<Session>, String> {
    let Ok(Some(goal)) = Goal::parse(input) else { return Ok(None) };
    let page = Page::open(page::today_file(pages))
        .map_err(|e| format!("Couldn't open today's page: {e}"))?;
    Ok(Some(Session::new(page, goal)))
}

/// Adds typed text to a short input line, up to `max_chars` characters.
fn push_limited(line: &mut String, text: &str, max_chars: usize) {
    let room = max_chars.saturating_sub(line.chars().count());
    line.extend(text.chars().filter(|c| !c.is_control()).take(room));
}

/// The writing column: its left edge and width, centred on the page.
fn column(painter: &Painter, rect: Rect) -> (f32, f32) {
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
        if session.page.is_dirty() && session.last_save.elapsed() >= AUTOSAVE_EVERY {
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
        ui.ctx().set_cursor_icon(CursorIcon::None);
        ui.painter().rect_filled(ui.max_rect(), 0.0, PAPER);
        let events = ui.input(|i| i.events.clone());
        match self.screen {
            Screen::Start { .. } => self.start_screen(ui, &events),
            Screen::Writing(_) => self.writing_screen(ui, &events),
        }
    }

    fn on_exit(&mut self) {
        if let Screen::Writing(session) = &mut self.screen {
            session.save();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        PAPER.to_normalized_gamma_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

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

    fn locked_session(name: &str, settings: Settings, goal: &str) -> (Harness<'static, WriterApp>, PathBuf) {
        let pages = std::env::temp_dir().join(format!("writer-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pages);
        let app_pages = pages.clone();
        let goal = goal.to_owned();
        let mut harness = Harness::builder()
            .build_eframe(move |_| WriterApp::new(settings, false, app_pages, Some(goal)));
        harness.state_mut().task_manager_in_front = || false;
        (harness, pages)
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

        // Goal reached: no longer on top, closing allowed, Esc leaves.
        std::thread::sleep(Duration::from_millis(2300).saturating_sub(started.elapsed()));
        let commands = frame(&mut h, Some(true), false, false, None);
        assert!(commands.contains(&ViewportCommand::WindowLevel(WindowLevel::Normal)));
        assert!(!frame(&mut h, Some(false), false, true, None).iter().any(|c| *c == CancelClose || *c == Focus));
        assert!(frame(&mut h, Some(true), false, false, press(Key::Escape)).contains(&Close));

        let saved = std::fs::read_to_string(page::today_file(&pages)).unwrap();
        assert_eq!(saved, "Hej æøå @ é");
        let _ = std::fs::remove_dir_all(&pages);
    }

    #[test]
    fn panic_sentence_releases_the_lock() {
        let (mut h, pages) = locked_session("panic", Settings { panic_exit: true }, "100");
        frame(&mut h, Some(true), false, false, Some(Event::Text("Some words".to_owned())));
        frame(&mut h, Some(true), false, false, press(Key::Escape));
        frame(&mut h, Some(true), false, false, Some(Event::Text("I choose to stop".to_owned())));
        assert!(!frame(&mut h, Some(true), false, false, press(Key::Enter)).contains(&ViewportCommand::Close));
        frame(&mut h, Some(true), false, false, Some(Event::Text(PANIC_SENTENCE["I choose to stop".len()..].to_owned())));
        assert!(frame(&mut h, Some(true), false, false, press(Key::Enter)).contains(&ViewportCommand::Close));
        assert!(!frame(&mut h, Some(true), false, true, None).contains(&ViewportCommand::CancelClose));
        assert_eq!(std::fs::read_to_string(page::today_file(&pages)).unwrap(), "Some words");
        let _ = std::fs::remove_dir_all(&pages);
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

        let mut harness = Harness::builder()
            .with_size([1280.0, 800.0])
            .wgpu()
            .build_eframe(|cc| {
                crate::load_typewriter_font(&cc.egui_ctx);
                WriterApp::new(Settings::default(), true, pages.clone(), None)
            });
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

        let mut harness = Harness::builder()
            .with_size([1280.0, 800.0])
            .wgpu()
            .build_eframe(|cc| {
                crate::load_typewriter_font(&cc.egui_ctx);
                WriterApp::new(Settings { panic_exit: true }, true, pages.clone(), None)
            });
        harness.run_steps(2);
        harness.event(Event::Text("1000".to_owned()));
        harness.key_press(Key::Enter);
        harness.event(Event::Text(THIRD.to_owned()));
        harness.key_press(Key::Escape);
        harness.event(Event::Text("I choose to stop".to_owned()));
        shot(&mut harness, "4-panic-exit");

        let _ = std::fs::remove_dir_all(&pages);
    }
}
