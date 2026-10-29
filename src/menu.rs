use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate};
use eframe::egui::{self, Align2, CursorIcon, Event, FontId, Key, MouseWheelUnit, Pos2, Rect, Role, Sense, WidgetInfo, pos2, vec2};

use crate::app::{FAINT, INK, LINE_SCROLL, NOTE_SIZE, TEXT_SIZE, column};
use crate::page::{self, Listed, Title};

/// How long a copy's name can be, in characters.
const NAME_CHARS: usize = 60;
const ROW_HEIGHT: f32 = TEXT_SIZE * 1.6;

/// The menu on the start screen: earlier pages and saved copies, to read and
/// to save a named copy of. Nothing in it can change a page.
pub struct Menu {
    pages: PathBuf,
    items: Vec<Item>,
    /// The line with the caret bar, moved by Up/Down or the pointer.
    selected: usize,
    scroll: f32,
    reading: Option<Reading>,
    /// Opens a folder in Explorer or Finder. A field so tests can stand in for it.
    pub open_folder: fn(&Path),
}

enum Item {
    Page(Listed),
    OpenFolder,
}

struct Reading {
    listed: Listed,
    text: String,
    scroll: f32,
    /// The name being typed, while saving a copy.
    naming: Option<Naming>,
    /// Where the last copy went.
    note: Option<String>,
}

#[derive(Default)]
struct Naming {
    name: String,
    /// Set by Enter on a name that's taken; another Enter replaces that copy.
    replace: bool,
    problem: Option<String>,
}

/// Whether the menu is still open after a frame.
#[derive(PartialEq, Eq, Debug)]
pub enum Outcome {
    Open,
    Closed,
}

impl Menu {
    pub fn open(pages: &Path) -> Self {
        Self {
            pages: pages.to_owned(),
            items: items(pages),
            selected: 0,
            scroll: 0.0,
            reading: None,
            open_folder: open_in_file_manager,
        }
    }

    pub fn show(&mut self, ui: &egui::Ui, events: &[Event]) -> Outcome {
        if self.reading.is_some() {
            self.reading_screen(ui, events);
            Outcome::Open
        } else {
            self.list_screen(ui, events)
        }
    }

    fn list_screen(&mut self, ui: &egui::Ui, events: &[Event]) -> Outcome {
        let rect = ui.max_rect();
        let (left, width) = column(ui.painter(), rect);
        let (top, bottom) = (rect.top() + 88.0, rect.bottom() - 72.0);
        let last = self.items.len() - 1;
        let mut open = None;
        let mut moved = false;
        for event in events {
            match event {
                Event::Key { key, pressed: true, .. } => {
                    moved = true;
                    match key {
                        Key::ArrowDown => self.selected = (self.selected + 1).min(last),
                        Key::ArrowUp => self.selected = self.selected.saturating_sub(1),
                        Key::Home => self.selected = 0,
                        Key::End => self.selected = last,
                        Key::Enter => open = Some(self.selected),
                        Key::Escape => return Outcome::Closed,
                        _ => moved = false,
                    }
                }
                Event::MouseWheel { unit, delta, .. } => self.scroll -= delta.y * wheel_step(*unit, bottom - top),
                _ => {}
            }
        }

        // Where each line goes, from the top of the list.
        let lines = self.lines();
        let mut y = 0.0;
        let placed: Vec<(Line, f32)> = lines
            .into_iter()
            .map(|line| {
                let at = y;
                y += line.height();
                (line, at)
            })
            .collect();
        let visible = bottom - top;
        if moved && let Some((_, at)) = placed.iter().find(|(line, _)| *line == Line::Row(self.selected)) {
            // Keep the selected line in view.
            self.scroll = self.scroll.min(*at).max(at + ROW_HEIGHT - visible);
        }
        self.scroll = self.scroll.clamp(0.0, (y - visible).max(0.0));

        let painter = ui.painter();
        painter.text(pos2(left, rect.top() + 32.0), Align2::LEFT_TOP, "Menu", FontId::monospace(NOTE_SIZE), INK);
        let list = Rect::from_x_y_ranges(rect.x_range(), top..=bottom);
        let clipped = painter.with_clip_rect(list);
        let pointer_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        let today = Local::now().date_naive();
        for (line, at) in &placed {
            let y = top + at - self.scroll;
            if y + line.height() < top || y > bottom {
                continue;
            }
            match *line {
                Line::Heading(text) => {
                    clipped.text(pos2(left, y + 6.0), Align2::LEFT_TOP, text, FontId::monospace(NOTE_SIZE), FAINT);
                }
                Line::Gap => {}
                Line::Row(index) => {
                    let (label, words) = row_text(&self.items[index], &self.pages, today);
                    let row = Rect::from_min_max(pos2(left - 24.0, y), pos2(left + width, y + ROW_HEIGHT));
                    let response = ui.interact(row.intersect(list), ui.scope_id().with(("menu row", index)), Sense::click());
                    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        if pointer_moved {
                            self.selected = index;
                        }
                    }
                    if response.clicked() {
                        open = Some(index);
                    }
                    let font = FontId::monospace(TEXT_SIZE);
                    clipped.text(pos2(left, y + 4.0), Align2::LEFT_TOP, &label, font.clone(), INK);
                    if let Some(words) = words {
                        clipped.text(pos2(left + width, y + 4.0), Align2::RIGHT_TOP, words, font, FAINT);
                    }
                    if index == self.selected {
                        let bar = Rect::from_min_size(pos2(left - 14.0, y + 4.0), vec2(2.0, TEXT_SIZE * 1.15));
                        clipped.rect_filled(bar, 0.0, FAINT);
                    }
                }
            }
        }

        let back = link(ui, rect.center_bottom() - vec2(0.0, 24.0), Align2::CENTER_BOTTOM, "Esc to go back", "menu back");
        if back.clicked() {
            return Outcome::Closed;
        }
        match open.map(|index| &self.items[index]) {
            Some(Item::Page(listed)) => self.reading = Some(Reading::new(listed.clone())),
            Some(Item::OpenFolder) => (self.open_folder)(&self.pages),
            None => {}
        }
        Outcome::Open
    }

    /// The list's lines: headings, gaps and items, in order.
    fn lines(&self) -> Vec<Line> {
        let is_day = |item: &Item| matches!(item, Item::Page(Listed { title: Title::Day(_), .. }));
        let is_copy = |item: &Item| matches!(item, Item::Page(Listed { title: Title::Copy(_), .. }));
        let mut lines = vec![Line::Heading(if self.items.iter().any(is_day) {
            "Your pages"
        } else {
            "Nothing written yet."
        })];
        let mut in_copies = false;
        for (index, item) in self.items.iter().enumerate() {
            if is_copy(item) && !in_copies {
                in_copies = true;
                lines.extend([Line::Gap, Line::Heading("Saved copies")]);
            }
            if matches!(item, Item::OpenFolder) {
                lines.push(Line::Gap);
            }
            lines.push(Line::Row(index));
        }
        lines
    }

    fn reading_screen(&mut self, ui: &egui::Ui, events: &[Event]) {
        let Some(reading) = &mut self.reading else { return };
        let rect = ui.max_rect();
        let (left, width) = column(ui.painter(), rect);
        let (top, bottom) = (rect.top() + 80.0, rect.bottom() - 104.0);
        let visible = bottom - top;
        let mut back = false;
        let mut save = false;
        for event in events {
            match (&mut reading.naming, event) {
                (_, Event::MouseWheel { unit, delta, .. }) => reading.scroll -= delta.y * wheel_step(*unit, visible),
                (Some(naming), Event::Text(text)) => {
                    let room = NAME_CHARS.saturating_sub(naming.name.chars().count());
                    naming.name.extend(text.chars().filter(|&c| page::allowed_in_name(c)).take(room));
                    (naming.replace, naming.problem) = (false, None);
                }
                (Some(naming), Event::Key { key, pressed: true, .. }) => match key {
                    Key::Backspace => {
                        naming.name.pop();
                        (naming.replace, naming.problem) = (false, None);
                    }
                    Key::Enter => save = true,
                    Key::Escape => reading.naming = None,
                    _ => {}
                },
                (None, Event::Key { key, pressed: true, modifiers, .. }) => match key {
                    Key::ArrowDown => reading.scroll += LINE_SCROLL,
                    Key::ArrowUp => reading.scroll -= LINE_SCROLL,
                    Key::PageDown => reading.scroll += visible * 0.8,
                    Key::PageUp => reading.scroll -= visible * 0.8,
                    Key::Home => reading.scroll = 0.0,
                    Key::End => reading.scroll = f32::MAX,
                    Key::S if modifiers.command => {
                        reading.naming = Some(Naming::default());
                        reading.note = None;
                    }
                    Key::Escape => back = true,
                    _ => {}
                },
                _ => {}
            }
        }
        if save {
            reading.save(&self.pages);
        }

        let painter = ui.painter();
        let note = FontId::monospace(NOTE_SIZE);
        let heading = format!("{} · {}", heading(&reading.listed, Local::now().date_naive()), words(reading.listed.words));
        painter.text(pos2(left, rect.top() + 32.0), Align2::LEFT_TOP, heading, note.clone(), FAINT);

        let galley = painter.layout(reading.text.clone(), FontId::monospace(TEXT_SIZE), INK, width + 0.5);
        reading.scroll = reading.scroll.clamp(0.0, (galley.size().y - visible).max(0.0));
        let clipped = painter.with_clip_rect(Rect::from_x_y_ranges(rect.x_range(), top..=bottom));
        if reading.text.trim().is_empty() {
            clipped.text(pos2(left, top), Align2::LEFT_TOP, "This page is empty.", note.clone(), FAINT);
        }
        clipped.galley(pos2(left, top - reading.scroll), galley, INK);

        let base = rect.center_bottom() - vec2(0.0, 24.0);
        if let Some(naming) = &reading.naming {
            let line = painter.layout_no_wrap(format!("Save as: {}", naming.name), note.clone(), INK);
            let pos = pos2(left, base.y - 48.0 - line.size().y);
            let caret = Rect::from_min_size(pos + vec2(line.size().x + 1.0, 0.0), vec2(2.0, line.size().y));
            painter.galley(pos, line, INK);
            painter.rect_filled(caret, 0.0, FAINT);
            let hint = match (&naming.problem, page::copy_path(&self.pages, &naming.name)) {
                (Some(problem), _) => problem.clone(),
                (None, Ok(path)) => format!("Enter saves it as {}. Esc cancels.", path.display()),
                (None, Err(_)) => "A name for the copy. Esc cancels.".to_owned(),
            };
            painter.text(pos2(left, base.y - 24.0), Align2::LEFT_BOTTOM, hint, note, FAINT);
        } else {
            if let Some(saved) = &reading.note {
                painter.text(base - vec2(0.0, 24.0), Align2::CENTER_BOTTOM, saved, note.clone(), FAINT);
            }
            // "Save a copy · Back", centred along the bottom.
            let separator = " · ";
            let width_of = |text: &str| painter.layout_no_wrap(text.to_owned(), note.clone(), FAINT).size().x;
            let total = width_of("Save a copy") + width_of(separator) + width_of("Back");
            let mut x = base.x - total / 2.0;
            if link(ui, pos2(x, base.y), Align2::LEFT_BOTTOM, "Save a copy", "menu save").clicked() {
                reading.naming = Some(Naming::default());
                reading.note = None;
            }
            x += width_of("Save a copy");
            painter.text(pos2(x, base.y), Align2::LEFT_BOTTOM, separator, note.clone(), FAINT);
            x += width_of(separator);
            back |= link(ui, pos2(x, base.y), Align2::LEFT_BOTTOM, "Back", "menu reading back").clicked();
        }

        if back {
            self.reading = None;
            // A copy may have been saved; it belongs in the list now.
            self.items = items(&self.pages);
            self.selected = self.selected.min(self.items.len() - 1);
        }
    }
}

impl Reading {
    fn new(listed: Listed) -> Self {
        Self { text: page::read_text(&listed.path), listed, scroll: 0.0, naming: None, note: None }
    }

    /// Saves a copy under the typed name, unless the name is taken and this
    /// isn't the second Enter.
    fn save(&mut self, pages: &Path) {
        let Some(naming) = &mut self.naming else { return };
        let path = match page::copy_path(pages, &naming.name) {
            Ok(path) => path,
            Err(problem) => {
                naming.problem = Some(problem.to_owned());
                return;
            }
        };
        if path.exists() && !naming.replace {
            naming.replace = true;
            let name = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
            naming.problem = Some(format!("There's already a copy called {name}. Enter again to replace it."));
            return;
        }
        match page::save_copy(&self.listed.path, &path) {
            Ok(()) => {
                self.naming = None;
                self.note = Some(format!("Saved as {}", path.display()));
            }
            Err(e) => naming.problem = Some(format!("Couldn't save the copy: {e}")),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Line {
    Heading(&'static str),
    Gap,
    Row(usize),
}

impl Line {
    fn height(self) -> f32 {
        match self {
            Line::Heading(_) => 30.0,
            Line::Gap => 22.0,
            Line::Row(_) => ROW_HEIGHT,
        }
    }
}

/// Pages newest first, then saved copies, then the folder.
fn items(pages: &Path) -> Vec<Item> {
    let listed = page::day_pages(pages).into_iter().chain(page::saved_copies(pages));
    listed.map(Item::Page).chain([Item::OpenFolder]).collect()
}

/// A line of the list: what it is, and its word count if it's a page.
fn row_text(item: &Item, pages: &Path, today: NaiveDate) -> (String, Option<String>) {
    match item {
        Item::Page(listed @ Listed { title: Title::Day(date), .. }) => {
            let date = date.format("%-d %B %Y");
            (format!("{:<12}{date}", day_name(listed, today)), Some(words(listed.words)))
        }
        Item::Page(listed @ Listed { title: Title::Copy(name), .. }) => {
            let name = if name.chars().count() > 40 { format!("{}…", name.chars().take(39).collect::<String>()) } else { name.clone() };
            (name, Some(words(listed.words)))
        }
        Item::OpenFolder => {
            let folder = pages.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
            (format!("Open the {folder} folder"), None)
        }
    }
}

/// "Today", "Yesterday", or the weekday within the last week; otherwise
/// nothing, and the date says it.
fn day_name(listed: &Listed, today: NaiveDate) -> String {
    let Title::Day(date) = listed.title else { return String::new() };
    match (today - date).num_days() {
        0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        2..=6 => date.format("%A").to_string(),
        _ => String::new(),
    }
}

/// The top line while reading: "Thursday, 22 October 2026" or the copy's name.
fn heading(listed: &Listed, today: NaiveDate) -> String {
    match &listed.title {
        Title::Day(date) => match day_name(listed, today) {
            name if name.is_empty() => date.format("%-d %B %Y").to_string(),
            name => format!("{name}, {}", date.format("%-d %B %Y")),
        },
        Title::Copy(name) => name.clone(),
    }
}

fn words(count: usize) -> String {
    if count == 1 { "1 word".to_owned() } else { format!("{count} words") }
}

fn wheel_step(unit: MouseWheelUnit, page: f32) -> f32 {
    match unit {
        MouseWheelUnit::Point => 1.0,
        MouseWheelUnit::Line => LINE_SCROLL,
        MouseWheelUnit::Page => page,
    }
}

/// Faint text that can be clicked: it darkens under the pointer.
pub fn link(ui: &egui::Ui, pos: Pos2, anchor: Align2, text: &str, id: &str) -> egui::Response {
    let font = FontId::monospace(NOTE_SIZE);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER);
    let rect = anchor.anchor_size(pos, galley.size());
    let response = ui.interact(rect.expand(4.0), ui.scope_id().with(id), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, text));
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    ui.painter().galley(rect.min, galley, if response.hovered() { INK } else { FAINT });
    response
}

/// Opens a folder in Explorer on Windows, Finder on a Mac.
fn open_in_file_manager(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(dir).spawn();
}

#[cfg(test)]
impl Menu {
    /// The list as it reads, line by line.
    pub fn rows(&self) -> Vec<String> {
        let today = Local::now().date_naive();
        self.items.iter().map(|item| row_text(item, &self.pages, today).0).collect()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The text of the page being read, if one is open.
    pub fn reading(&self) -> Option<&str> {
        self.reading.as_ref().map(|reading| reading.text.as_str())
    }

    /// The name being typed for a copy, and what the line says about it.
    pub fn naming(&self) -> Option<(&str, Option<&str>)> {
        let naming = self.reading.as_ref()?.naming.as_ref()?;
        Some((naming.name.as_str(), naming.problem.as_deref()))
    }
}
