//! The screen for onlookers (Ctrl+Space): an Ubuntu terminal under Windows
//! Terminal, busy with routine admin work. See "Onlookers" in DESIGN.md.
//!
//! Everything on it is played from scripts against the terminal's own clock,
//! which only runs while it's showing.

mod apt;
mod htop;
mod journal;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontData, FontFamily, FontId, Painter, Rect, vec2,
};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

// Windows Terminal's "Ubuntu" colour scheme, which the WSL profile uses.
const BACKGROUND: Color32 = Color32::from_rgb(0x30, 0x0A, 0x24);
const FOREGROUND: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
const CURSOR: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
const BLACK: Color32 = Color32::from_rgb(0x17, 0x14, 0x21);
const GREEN: Color32 = Color32::from_rgb(0x26, 0xA2, 0x69);
const CYAN: Color32 = Color32::from_rgb(0x3A, 0x96, 0xDD);
const BLUE: Color32 = Color32::from_rgb(0x00, 0x37, 0xDA);
const RED: Color32 = Color32::from_rgb(0xC2, 0x1A, 0x23);
const YELLOW: Color32 = Color32::from_rgb(0xA2, 0x73, 0x4C);
const BRIGHT_BLACK: Color32 = Color32::from_rgb(0x76, 0x76, 0x76);
const BRIGHT_BLUE: Color32 = Color32::from_rgb(0x08, 0x45, 0x8F);
const BRIGHT_RED: Color32 = Color32::from_rgb(0xC0, 0x1C, 0x28);
const BRIGHT_WHITE: Color32 = Color32::from_rgb(0xF2, 0xF2, 0xF2);
/// Colour 185 of the 256, which journalctl uses for warnings.
const WARNING: Color32 = Color32::from_rgb(0xD7, 0xD7, 0x5F);

/// Ubuntu Mono at 13 pt, as the profile asks for.
const FONT_SIZE: f32 = 13.0 * 96.0 / 72.0;
/// Windows Terminal's default padding around the text.
const PADDING: f32 = 8.0;
/// The Windows caret blink time.
const BLINK: Duration = Duration::from_millis(530);
/// Lines kept above the screen; far more than fit on it.
const SCROLLBACK: usize = 1000;
/// The font family the terminal is drawn in.
const FAMILY: &str = "terminal";

/// Whether Ctrl+Space brings the terminal up. Not on a Mac, where the key
/// switches keyboard input sources.
pub const OFFERED: bool = cfg!(not(target_os = "macos"));

/// The window title Windows Terminal shows for the profile.
pub const TITLE: &str = "Ubuntu";
/// The colour behind everything while the terminal shows.
pub const CLEAR: Color32 = BACKGROUND;

/// The colours of a run of text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    pub fg: Color32,
    pub bg: Option<Color32>,
}

impl Style {
    pub const PLAIN: Self = Self::fg(FOREGROUND);

    pub const fn fg(fg: Color32) -> Self {
        Self { fg, bg: None }
    }

    pub const fn on(fg: Color32, bg: Color32) -> Self {
        Self { fg, bg: Some(bg) }
    }
}

/// One line of output, in runs of the same colours.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    spans: Vec<(String, Style)>,
}

impl Line {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new().with(text, Style::PLAIN)
    }

    pub fn with(mut self, text: impl Into<String>, style: Style) -> Self {
        self.push(&text.into(), style);
        self
    }

    pub fn push(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        match self.spans.last_mut() {
            Some((last, last_style)) if *last_style == style => last.push_str(text),
            _ => self.spans.push((text.to_owned(), style)),
        }
    }

    pub fn append(&mut self, other: Line) {
        for (text, style) in other.spans {
            self.push(&text, style);
        }
    }

    /// Length in characters, which is cells: there's nothing double-width.
    pub fn len(&self) -> usize {
        self.spans.iter().map(|(text, _)| text.chars().count()).sum()
    }

    #[cfg(test)]
    pub fn text(&self) -> String {
        self.spans.iter().map(|(text, _)| text.as_str()).collect()
    }

    /// Cut to at most `cols` characters.
    pub fn truncate(mut self, cols: usize) -> Self {
        let mut left = cols;
        self.spans.retain_mut(|(text, _)| {
            if left == 0 {
                return false;
            }
            if let Some((cut, _)) = text.char_indices().nth(left) {
                text.truncate(cut);
            }
            left -= text.chars().count();
            true
        });
        self
    }

    /// Filled with spaces in `style` out to `cols` characters.
    pub fn pad(mut self, cols: usize, style: Style) -> Self {
        let len = self.len();
        if len < cols {
            self.push(&" ".repeat(cols - len), style);
        }
        self
    }

    /// The rows the line takes up in a terminal `cols` wide. An empty line
    /// still takes one.
    fn rows(&self, cols: usize) -> Vec<Line> {
        let cols = cols.max(1);
        let mut rows = vec![Line::new()];
        let mut room = cols;
        for (text, style) in &self.spans {
            let mut rest = text.as_str();
            while !rest.is_empty() {
                if room == 0 {
                    rows.push(Line::new());
                    room = cols;
                }
                let cut = rest.char_indices().nth(room).map_or(rest.len(), |(i, _)| i);
                let (now, later) = rest.split_at(cut);
                rows.last_mut().unwrap().push(now, *style);
                room -= now.chars().count();
                rest = later;
            }
        }
        rows
    }
}

/// The machine the terminal is on: WSL's Ubuntu on this computer.
#[derive(Clone, Debug)]
pub struct Machine {
    /// The Linux user, the Windows one in lower case.
    pub user: String,
    /// WSL takes the computer's name.
    pub host: String,
    /// When the WSL machine started, for `uptime` and htop.
    pub booted: DateTime<Local>,
}

impl Machine {
    pub fn here(seed: u64) -> Self {
        let user = std::env::var("USERNAME").map_or_else(|_| "user".to_owned(), |name| name.to_lowercase());
        let host = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "localhost".to_owned());
        // Up a few days: WSL keeps running in the background.
        let up = chrono::Duration::minutes(3 * 24 * 60 + (seed % (2 * 24 * 60)) as i64);
        Self { user, host, booted: Local::now() - up }
    }

    /// `user@host:~$ ` in the colours of Ubuntu's prompt, bold green and
    /// blue, which Windows Terminal draws as the bright ones.
    pub fn prompt(&self) -> Line {
        Line::new()
            .with(format!("{}@{}", self.user, self.host), Style::fg(GREEN))
            .with(":", Style::PLAIN)
            .with("~", Style::fg(BRIGHT_BLUE))
            .with("$ ", Style::PLAIN)
    }
}

/// A small, fast random number generator (SplitMix64); nothing here needs
/// more, and a seed makes the scripts repeatable in tests.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `low..high`.
    pub fn range(&mut self, low: u64, high: u64) -> u64 {
        low + self.next() % (high - low).max(1)
    }

    /// A number in `0.0..1.0`.
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range(0, items.len() as u64) as usize]
    }
}

/// A number from `seed` and `parts` that's the same every time: for values
/// that are a function of time, like htop's, rather than a sequence.
pub fn noise(seed: u64, parts: &[u64]) -> f64 {
    let mut rng = Rng::new(parts.iter().fold(seed, |acc, part| acc.rotate_left(17) ^ part.wrapping_mul(0xA24B_AED4_963E_E407)));
    rng.next();
    rng.unit()
}

/// What a script does next, once its wait is over.
#[derive(Clone, Debug)]
pub enum Action {
    /// Adds to the line being written: the prompt, a typed letter, " Done".
    Text(String, Style),
    /// Ends the line being written.
    Newline,
    /// Writes a whole line.
    Line(Line),
    /// The line apt keeps along the bottom while it works, or none.
    Status(Option<Status>),
    /// Nothing; only the wait before it counts.
    Pause,
}

/// apt's bottom line: what it's downloading, then dpkg's progress bar.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Text(String),
    Progress(u32),
}

#[derive(Clone, Debug)]
pub struct Step {
    /// How long after the step before it.
    pub after: Duration,
    pub action: Action,
}

/// Steps being put together by a script.
#[derive(Default)]
pub struct Plan {
    steps: Vec<Step>,
    wait: Duration,
}

impl Plan {
    pub fn wait(&mut self, ms: u64) -> &mut Self {
        self.wait += Duration::from_millis(ms);
        self
    }

    fn act(&mut self, action: Action) -> &mut Self {
        let after = std::mem::take(&mut self.wait);
        self.steps.push(Step { after, action });
        self
    }

    pub fn print(&mut self, line: impl Into<Line>) -> &mut Self {
        self.act(Action::Line(line.into()))
    }

    pub fn text(&mut self, text: impl Into<String>, style: Style) -> &mut Self {
        self.act(Action::Text(text.into(), style))
    }

    pub fn newline(&mut self) -> &mut Self {
        self.act(Action::Newline)
    }

    pub fn status(&mut self, status: Option<Status>) -> &mut Self {
        self.act(Action::Status(status))
    }

    /// The prompt; a while later, `command` typed at a person's pace, then
    /// Enter.
    pub fn command(&mut self, rng: &mut Rng, machine: &Machine, idle_ms: (u64, u64), command: &str) -> &mut Self {
        for (text, style) in machine.prompt().spans {
            self.text(text, style);
        }
        self.wait(rng.range(idle_ms.0, idle_ms.1));
        for c in command.chars() {
            let pause = if c == ' ' && rng.chance(0.3) { rng.range(250, 700) } else { rng.range(45, 170) };
            self.wait(pause).text(c, Style::PLAIN);
        }
        self.wait(rng.range(200, 600)).newline()
    }

    /// A command and its prompt, already there: history from before.
    pub fn typed(&mut self, machine: &Machine, command: &str) -> &mut Self {
        self.print(machine.prompt().with(command, Style::PLAIN))
    }

    fn into_steps(mut self) -> Vec<Step> {
        // A wait left at the end still counts, and every plan takes some
        // time, so a script can never spin.
        if !self.wait.is_zero() || self.steps.iter().all(|step| step.after.is_zero()) {
            let after = self.wait.max(Duration::from_millis(1));
            self.steps.push(Step { after, action: Action::Pause });
        }
        self.steps
    }
}

impl From<String> for Line {
    fn from(text: String) -> Self {
        Line::plain(text)
    }
}

impl From<&str> for Line {
    fn from(text: &str) -> Self {
        Line::plain(text)
    }
}

/// What a script gets to work with when it's asked for more.
pub struct Cx<'a> {
    pub rng: &'a mut Rng,
    pub machine: &'a Machine,
    /// The time on the clock now, for log lines and `uptime`.
    pub now: DateTime<Local>,
    /// How long the feed has been running.
    pub clock: Duration,
    /// The terminal's width, as apt lays out its lists to it.
    pub cols: usize,
}

/// Something that writes to a terminal over time: apt, journalctl.
pub trait Script {
    /// The next stretch of steps. Runs forever.
    fn next(&mut self, cx: &mut Cx) -> Plan;
}

/// A terminal fed by a script: the lines so far, the one being written, and
/// the steps still to come.
pub struct Feed {
    script: Box<dyn Script>,
    rng: Rng,
    machine: Machine,
    queue: VecDeque<Step>,
    /// When the last step happened.
    at: Duration,
    lines: VecDeque<Line>,
    current: Line,
    status: Option<Status>,
    /// When something was last written, so the cursor holds still meanwhile.
    written: Duration,
    cols: usize,
}

impl Feed {
    pub fn new(script: Box<dyn Script>, machine: Machine, seed: u64) -> Self {
        Self {
            script,
            rng: Rng::new(seed),
            machine,
            queue: VecDeque::new(),
            at: Duration::ZERO,
            lines: VecDeque::new(),
            current: Line::new(),
            status: None,
            written: Duration::ZERO,
            cols: 120,
        }
    }

    /// Plays every step due by `clock`, which is `now` on the wall clock.
    pub fn run_to(&mut self, clock: Duration, now: DateTime<Local>) {
        loop {
            if self.queue.is_empty() {
                // Catching up, the steps happened a little while ago.
                let behind = chrono::Duration::from_std(clock.saturating_sub(self.at)).unwrap_or_default();
                let mut cx = Cx { rng: &mut self.rng, machine: &self.machine, now: now - behind, clock: self.at, cols: self.cols };
                self.queue.extend(self.script.next(&mut cx).into_steps());
            }
            let Some(step) = self.queue.front() else { return };
            let due = self.at + step.after;
            if due > clock {
                return;
            }
            let action = self.queue.pop_front().unwrap().action;
            self.at = due;
            self.apply(action);
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Text(text, style) => self.current.push(&text, style),
            Action::Newline => self.end_line(),
            Action::Line(line) => {
                self.current.append(line);
                self.end_line();
            }
            Action::Status(status) => self.status = status,
            Action::Pause => return,
        }
        self.written = self.at;
    }

    fn end_line(&mut self) {
        self.lines.push_back(std::mem::take(&mut self.current));
        if self.lines.len() > SCROLLBACK {
            self.lines.pop_front();
        }
    }

    /// The rows on screen, top to bottom, and where the cursor is in the
    /// last of them.
    fn screen(&self, cols: usize, rows: usize) -> (Vec<Line>, usize) {
        let rows = rows.max(1);
        let current = self.current.rows(cols);
        let cursor = current.last().map_or(0, Line::len).min(cols.saturating_sub(1));
        let mut shown: Vec<Line> = current.into_iter().rev().take(rows).collect();
        for line in self.lines.iter().rev() {
            if shown.len() >= rows {
                break;
            }
            let wrapped = line.rows(cols);
            shown.extend(wrapped.into_iter().rev().take(rows - shown.len()));
        }
        shown.reverse();
        (shown, cursor)
    }

    #[cfg(test)]
    pub fn all_text(&self) -> String {
        let mut text: Vec<String> = self.lines.iter().map(Line::text).collect();
        text.push(self.current.text());
        text.join("\n")
    }

    #[cfg(test)]
    pub fn status(&self) -> Option<&Status> {
        self.status.as_ref()
    }
}

/// What's on another monitor: htop, or the system log going by.
enum Cover {
    Top(htop::Top),
    Journal(Feed),
}

/// The terminal and its clock, kept between showings so it carries on where
/// it left off.
pub struct Terminal {
    shown: bool,
    /// How long it's been showing, all told.
    clock: Duration,
    last_tick: Option<Instant>,
    machine: Machine,
    seed: u64,
    main: Feed,
    /// The main screen starts this far in, with the update under way.
    head_start: Duration,
    covers: HashMap<usize, Cover>,
    font_loaded: bool,
}

impl Terminal {
    /// The terminal for this computer, different each time the app starts.
    pub fn here() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos() as u64);
        Self::new(Machine::here(seed), seed)
    }

    pub fn new(machine: Machine, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let head_start = Duration::from_millis(rng.range(25_000, 70_000));
        let main = Feed::new(Box::new(apt::Apt::new()), machine.clone(), rng.next());
        Self {
            shown: false,
            clock: Duration::ZERO,
            last_tick: None,
            machine,
            seed,
            main,
            head_start,
            covers: HashMap::new(),
            font_loaded: false,
        }
    }

    pub fn shown(&self) -> bool {
        self.shown
    }

    pub fn switch(&mut self) {
        self.shown = !self.shown;
        self.last_tick = None;
    }

    /// Moves the clock on by the time since the last frame it showed. A long
    /// gap (the computer asleep, say) only counts as a second.
    pub fn tick(&mut self, now: Instant) {
        if let Some(last) = self.last_tick {
            self.clock += now.saturating_duration_since(last).min(Duration::from_secs(1));
        }
        self.last_tick = Some(now);
    }

    /// Makes sure the terminal's font is there. Before anything is drawn in
    /// a frame, it's in use at once.
    pub fn load_font(&mut self, ctx: &egui::Context) {
        if !std::mem::replace(&mut self.font_loaded, true) {
            load_font(ctx);
        }
    }

    /// The main screen, where apt is running.
    pub fn show(&mut self, ui: &egui::Ui) {
        let clock = self.clock + self.head_start;
        let (painter, rect) = whole_window(ui);
        let painter = &painter;
        let grid = Grid::new(painter, rect);
        // apt lays out its lists to the width, so it needs it first.
        self.main.cols = grid.cols;
        self.main.run_to(clock, Local::now());
        // The cursor holds still while things are written, and blinks once
        // they stop.
        let quiet = clock.saturating_sub(self.main.written);
        let cursor_on = quiet < BLINK || (quiet.as_millis() / BLINK.as_millis()) % 2 == 0;
        grid.feed(painter, &self.main, cursor_on);
        ui.ctx().set_cursor_icon(CursorIcon::Text);
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }

    /// Another monitor's screen. `nth` counts the other monitors from zero:
    /// htop goes on the first, the log on the second, and so on in turn.
    pub fn show_cover(&mut self, ui: &egui::Ui, monitor: usize, nth: usize) {
        let (machine, seed) = (&self.machine, self.seed ^ (monitor as u64 + 1).wrapping_mul(0x5851_F42D));
        let cover = self.covers.entry(monitor).or_insert_with(|| {
            if nth % 2 == 0 {
                Cover::Top(htop::Top::new(machine.clone(), seed))
            } else {
                Cover::Journal(Feed::new(Box::new(journal::Journal::new()), machine.clone(), seed))
            }
        });
        let (painter, rect) = whole_window(ui);
        let painter = &painter;
        let grid = Grid::new(painter, rect);
        match cover {
            Cover::Top(top) => {
                for (row, line) in top.screen(grid.cols, grid.rows, self.clock, Local::now()).iter().enumerate() {
                    grid.line(painter, row, line);
                }
            }
            Cover::Journal(feed) => {
                feed.cols = grid.cols;
                feed.run_to(self.clock, Local::now());
                grid.feed(painter, feed, true);
            }
        }
        ui.ctx().set_cursor_icon(CursorIcon::Text);
    }
}

/// A painter for the whole window, edge to edge, with the terminal's
/// background already on it.
fn whole_window(ui: &egui::Ui) -> (Painter, Rect) {
    let rect = ui.ctx().content_rect();
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, BACKGROUND);
    (painter, rect)
}

/// The character cells of a screen.
struct Grid {
    origin: egui::Pos2,
    cell: egui::Vec2,
    cols: usize,
    rows: usize,
    font: FontId,
}

impl Grid {
    fn new(painter: &Painter, rect: Rect) -> Self {
        let font = FontId::new(FONT_SIZE, FontFamily::Name(FAMILY.into()));
        let width = painter.layout_no_wrap("M".to_owned(), font.clone(), FOREGROUND).size().x;
        let height = painter.fonts_mut(|fonts| fonts.row_height(&font));
        let inner = rect.shrink(PADDING);
        let cols = (inner.width() / width).floor().max(1.0) as usize;
        let rows = (inner.height() / height).floor().max(1.0) as usize;
        Self { origin: inner.min, cell: vec2(width, height), cols, rows, font }
    }

    fn cell(&self, col: usize, row: usize) -> Rect {
        let min = self.origin + vec2(col as f32 * self.cell.x, row as f32 * self.cell.y);
        Rect::from_min_size(min, self.cell)
    }

    /// A feed's lines from the top, its status line along the bottom.
    fn feed(&self, painter: &Painter, feed: &Feed, cursor_on: bool) {
        // dpkg keeps the bottom row for its progress bar; apt's download line
        // is written over the line the cursor is on.
        let progress = match feed.status {
            Some(Status::Progress(percent)) if self.rows > 1 => Some(percent),
            _ => None,
        };
        let (mut rows, cursor) = feed.screen(self.cols, self.rows - usize::from(progress.is_some()));
        if let (Some(Status::Text(text)), Some(last)) = (&feed.status, rows.last_mut()) {
            *last = Line::plain(text.as_str()).truncate(self.cols);
        }
        for (row, line) in rows.iter().enumerate() {
            self.line(painter, row, line);
        }
        if cursor_on && feed.status.is_none() {
            painter.rect_filled(self.cell(cursor, rows.len().saturating_sub(1)), 0.0, CURSOR);
        }
        if let Some(percent) = progress {
            self.line(painter, self.rows - 1, &progress_bar(percent, self.cols));
        }
    }

    /// Draws a row. Plain ASCII goes in runs; anything else is put in its
    /// own cell, in case it comes from a fallback font of another width.
    fn line(&self, painter: &Painter, row: usize, line: &Line) {
        let mut col = 0;
        for (text, style) in &line.spans {
            let len = text.chars().count();
            if let Some(bg) = style.bg {
                let span = self.cell(col, row).union(self.cell(col + len.max(1) - 1, row));
                painter.rect_filled(span, 0.0, bg);
            }
            let mut run = String::new();
            let mut run_start = col;
            for c in text.chars() {
                if c.is_ascii() {
                    run.push(c);
                } else {
                    self.text(painter, run_start, row, &std::mem::take(&mut run), style.fg);
                    self.text(painter, col, row, c.encode_utf8(&mut [0; 4]), style.fg);
                    run_start = col + 1;
                }
                col += 1;
            }
            self.text(painter, run_start, row, &run, style.fg);
        }
    }

    fn text(&self, painter: &Painter, col: usize, row: usize, text: &str, color: Color32) {
        if !text.trim().is_empty() {
            painter.text(self.cell(col, row).min, Align2::LEFT_TOP, text, self.font.clone(), color);
        }
    }
}

/// dpkg's progress along the bottom while apt installs, as Ubuntu colours
/// it: `Progress: [ 37%] [##########..........]`.
fn progress_bar(percent: u32, cols: usize) -> Line {
    let label = format!("Progress: [{percent:>3}%]");
    let width = cols.saturating_sub(label.len() + 3);
    let filled = width * percent.min(100) as usize / 100;
    Line::new()
        .with(label, Style::on(BLACK, GREEN))
        .with(format!(" [{}{}]", "#".repeat(filled), ".".repeat(width - filled)), Style::PLAIN)
}

/// Adds the terminal's font family: Ubuntu Mono if it's installed (the WSL
/// profile asks for it), else Consolas, with egui's own monospace fonts
/// behind it for any symbol those lack.
fn load_font(ctx: &egui::Context) {
    let family = || vec![InsertFontFamily { family: FontFamily::Name(FAMILY.into()), priority: FontPriority::Lowest }];
    if let Some(bytes) = font_file().and_then(|path| std::fs::read(path).ok()) {
        ctx.add_font(FontInsert::new("terminal: Ubuntu Mono", FontData::from_owned(bytes), family()));
    }
    let defaults = egui::FontDefinitions::default();
    for name in defaults.families.get(&FontFamily::Monospace).into_iter().flatten() {
        if let Some(data) = defaults.font_data.get(name) {
            ctx.add_font(FontInsert::new(&format!("terminal: {name}"), (**data).clone(), family()));
        }
    }
}

fn font_file() -> Option<PathBuf> {
    let windows = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    let system = Path::new(&windows).join("Fonts");
    // Fonts installed for just this user go in their own folder.
    let user = std::env::var_os("LOCALAPPDATA").map(|local| Path::new(&local).join("Microsoft\\Windows\\Fonts"));
    let names = ["UbuntuMono[wght].ttf", "UbuntuMono-Regular.ttf", "UbuntuMono-R.ttf"];
    let ubuntu = [user.as_deref(), Some(system.as_path())]
        .into_iter()
        .flatten()
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)));
    ubuntu.chain([system.join("consola.ttf")]).find(|path| path.is_file())
}

/// Sizes the way apt writes them: `73.0 kB`, `1,354 kB`, `10.7 MB`.
pub fn size(bytes: u64) -> String {
    let units = ["B", "kB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 10_000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if value < 100.0 && unit > 0 {
        format!("{value:.1} {}", units[unit])
    } else {
        format!("{} {}", grouped(value.round() as u64), units[unit])
    }
}

/// `9846` as `9,846`.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine() -> Machine {
        let booted = Local::now() - chrono::Duration::days(3);
        Machine { user: "peter".to_owned(), host: "DESKTOP-4KQ2".to_owned(), booted }
    }

    #[test]
    fn sizes_read_like_apt() {
        assert_eq!(size(4_928), "4,928 B");
        assert_eq!(size(73_000), "73.0 kB");
        assert_eq!(size(1_354_000), "1,354 kB");
        assert_eq!(size(10_700_000), "10.7 MB");
        assert_eq!(size(128_400_000), "128 MB");
        assert_eq!(grouped(9_846), "9,846");
        assert_eq!(grouped(41_893), "41,893");
    }

    #[test]
    fn lines_wrap_like_a_terminal() {
        let line = Line::new().with("abcdef", Style::PLAIN).with("ghij", Style::fg(GREEN));
        let rows: Vec<String> = line.rows(4).iter().map(Line::text).collect();
        assert_eq!(rows, ["abcd", "efgh", "ij"]);
        assert_eq!(Line::new().rows(80).len(), 1, "an empty line is still a row");
        assert_eq!(line.clone().truncate(7).text(), "abcdefg");
        assert_eq!(line.pad(12, Style::PLAIN).len(), 12);
    }

    #[test]
    fn the_screen_shows_the_end_of_the_feed() {
        struct Count(u32);
        impl Script for Count {
            fn next(&mut self, cx: &mut Cx) -> Plan {
                self.0 += 1;
                let mut plan = Plan::default();
                plan.wait(1000).print(format!("line {} on {}", self.0, cx.machine.host));
                plan
            }
        }
        let mut feed = Feed::new(Box::new(Count(0)), machine(), 1);
        feed.run_to(Duration::from_millis(2500), Local::now());
        assert_eq!(feed.all_text(), "line 1 on DESKTOP-4KQ2\nline 2 on DESKTOP-4KQ2\n");
        feed.run_to(Duration::from_secs(10), Local::now());
        let (rows, cursor) = feed.screen(80, 4);
        let rows: Vec<String> = rows.iter().map(Line::text).collect();
        assert_eq!(rows, ["line 8 on DESKTOP-4KQ2", "line 9 on DESKTOP-4KQ2", "line 10 on DESKTOP-4KQ2", ""]);
        assert_eq!(cursor, 0, "the cursor waits at the start of the next line");
    }

    #[test]
    fn a_command_is_typed_after_the_prompt() {
        let mut rng = Rng::new(7);
        let mut plan = Plan::default();
        plan.command(&mut rng, &machine(), (1000, 1001), "df -h");
        let steps = plan.into_steps();
        let typed: String = steps
            .iter()
            .filter_map(|step| match &step.action {
                Action::Text(text, _) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(typed, "peter@DESKTOP-4KQ2:~$ df -h");
        assert!(matches!(steps.last().unwrap().action, Action::Newline));
        let total: Duration = steps.iter().map(|step| step.after).sum();
        assert!(total > Duration::from_millis(1200), "typing takes a person's time: {total:?}");
    }

    /// Renders the terminal's screens to `target/screenshots`. Needs a GPU:
    /// `cargo test -- --ignored terminal_screenshots`
    #[test]
    #[ignore]
    fn terminal_screenshots() {
        use egui_kittest::Harness;
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("screenshots");
        std::fs::create_dir_all(&out).unwrap();
        let here = Machine::here(5);
        // The main screen as it first appears, and later with dpkg at work.
        let mut later = Terminal::new(here.clone(), 11);
        later.main.cols = 182;
        while !matches!(later.main.status, Some(Status::Progress(20..=60))) {
            later.clock += Duration::from_millis(250);
            later.main.run_to(later.clock + later.head_start, Local::now());
        }
        let mut covers = [Terminal::new(here.clone(), 11), Terminal::new(here.clone(), 11)];
        for cover in &mut covers {
            cover.clock = Duration::from_secs(90);
        }
        let [top, journal] = covers;
        let shots: [(&str, Terminal, Option<usize>); 4] = [
            ("13-terminal", Terminal::new(here, 11), None),
            ("14-terminal-dpkg", later, None),
            ("15-htop", top, Some(0)),
            ("16-journal", journal, Some(1)),
        ];
        for (name, mut terminal, cover) in shots {
            let mut harness = Harness::builder().with_size([1600.0, 900.0]).wgpu().build_ui(move |ui| {
                terminal.load_font(ui.ctx());
                match cover {
                    None => terminal.show(ui),
                    Some(nth) => terminal.show_cover(ui, nth, nth),
                }
            });
            harness.run_steps(2);
            harness.render().unwrap().save(out.join(format!("{name}.png"))).unwrap();
        }
    }

    #[test]
    fn the_progress_bar_fills_the_width() {
        let bar = progress_bar(37, 60);
        assert_eq!(bar.len(), 60);
        assert!(bar.text().starts_with("Progress: [ 37%] [#"));
        assert_eq!(progress_bar(100, 60).text().matches('.').count(), 0);
    }
}
