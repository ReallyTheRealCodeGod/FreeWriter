use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, NaiveDate};
use eframe::egui::{self, Align2, CursorIcon, Event, FontId, Key, MouseWheelUnit, Pos2, Rect, Role, Sense, WidgetInfo, pos2, vec2};

use crate::app::{FAINT, INK, LINE_SCROLL, NOTE_SIZE, TEXT_SIZE, column};
use crate::page::{self, Listed};

const ROW_HEIGHT: f32 = TEXT_SIZE * 1.6;
/// Where the "when" column starts in a row, in characters.
const WHEN_COLUMN: usize = 32;

/// The menu on the start screen: the pages written so far, to read and to
/// choose one to write on. Nothing in it can change a page.
pub struct Menu {
    pages: PathBuf,
    items: Vec<Item>,
    /// The line with the caret bar, moved by Up/Down or the pointer.
    selected: usize,
    scroll: f32,
    reading: Option<Reading>,
    /// Opens a folder in Explorer. A field so tests can stand in for it.
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
}

/// What became of the menu after a frame.
#[derive(PartialEq, Eq, Debug)]
pub enum Outcome {
    Open,
    Closed,
    /// "Write on it": the next session goes on this page.
    WriteOn(Listed),
}

impl Menu {
    pub fn open(pages: &Path) -> Self {
        Self {
            pages: pages.to_owned(),
            items: items(pages),
            selected: 0,
            scroll: 0.0,
            reading: None,
            open_folder: open_in_explorer,
        }
    }

    pub fn show(&mut self, ui: &egui::Ui, events: &[Event]) -> Outcome {
        if self.reading.is_some() {
            self.reading_screen(ui, events)
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

    /// The list's lines: a heading, the pages, and the folder below a gap.
    fn lines(&self) -> Vec<Line> {
        let any = self.items.iter().any(|item| matches!(item, Item::Page(_)));
        let mut lines = vec![Line::Heading(if any { "Your pages" } else { "Nothing written yet." })];
        for (index, item) in self.items.iter().enumerate() {
            if matches!(item, Item::OpenFolder) {
                lines.push(Line::Gap);
            }
            lines.push(Line::Row(index));
        }
        lines
    }

    fn reading_screen(&mut self, ui: &egui::Ui, events: &[Event]) -> Outcome {
        let Some(reading) = &mut self.reading else { return Outcome::Open };
        let rect = ui.max_rect();
        let (left, width) = column(ui.painter(), rect);
        let (top, bottom) = (rect.top() + 80.0, rect.bottom() - 104.0);
        let visible = bottom - top;
        let mut back = false;
        let mut write = false;
        for event in events {
            match event {
                Event::MouseWheel { unit, delta, .. } => reading.scroll -= delta.y * wheel_step(*unit, visible),
                Event::Key { key, pressed: true, .. } => match key {
                    Key::ArrowDown => reading.scroll += LINE_SCROLL,
                    Key::ArrowUp => reading.scroll -= LINE_SCROLL,
                    Key::PageDown => reading.scroll += visible * 0.8,
                    Key::PageUp => reading.scroll -= visible * 0.8,
                    Key::Home => reading.scroll = 0.0,
                    Key::End => reading.scroll = f32::MAX,
                    Key::Enter => write = true,
                    Key::Escape => back = true,
                    _ => {}
                },
                _ => {}
            }
        }

        let painter = ui.painter();
        let note = FontId::monospace(NOTE_SIZE);
        let heading = format!("{} · {}", reading.listed.name, words(reading.listed.words));
        painter.text(pos2(left, rect.top() + 32.0), Align2::LEFT_TOP, heading, note.clone(), FAINT);

        let galley = painter.layout(reading.text.clone(), FontId::monospace(TEXT_SIZE), INK, width + 0.5);
        reading.scroll = reading.scroll.clamp(0.0, (galley.size().y - visible).max(0.0));
        let clipped = painter.with_clip_rect(Rect::from_x_y_ranges(rect.x_range(), top..=bottom));
        if reading.text.trim().is_empty() {
            clipped.text(pos2(left, top), Align2::LEFT_TOP, "This page is empty.", note.clone(), FAINT);
        }
        clipped.galley(pos2(left, top - reading.scroll), galley, INK);

        // "Write on it · Back", centred along the bottom.
        let base = rect.center_bottom() - vec2(0.0, 24.0);
        let separator = " · ";
        let width_of = |text: &str| painter.layout_no_wrap(text.to_owned(), note.clone(), FAINT).size().x;
        let total = width_of("Write on it") + width_of(separator) + width_of("Back");
        let mut x = base.x - total / 2.0;
        write |= link(ui, pos2(x, base.y), Align2::LEFT_BOTTOM, "Write on it", "menu write").clicked();
        x += width_of("Write on it");
        painter.text(pos2(x, base.y), Align2::LEFT_BOTTOM, separator, note.clone(), FAINT);
        x += width_of(separator);
        back |= link(ui, pos2(x, base.y), Align2::LEFT_BOTTOM, "Back", "menu reading back").clicked();

        if write {
            return Outcome::WriteOn(reading.listed.clone());
        }
        if back {
            self.reading = None;
        }
        Outcome::Open
    }
}

impl Reading {
    fn new(listed: Listed) -> Self {
        Self { text: page::read_text(&listed.path), listed, scroll: 0.0 }
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

/// The pages, written to last first, then the folder.
fn items(pages: &Path) -> Vec<Item> {
    page::all_pages(pages).into_iter().map(Item::Page).chain([Item::OpenFolder]).collect()
}

/// A line of the list: what it is, and its word count if it's a page.
fn row_text(item: &Item, pages: &Path, today: NaiveDate) -> (String, Option<String>) {
    match item {
        Item::Page(listed) => {
            let room = WHEN_COLUMN - 2;
            let name = if listed.name.chars().count() > room {
                format!("{}…", listed.name.chars().take(room - 1).collect::<String>())
            } else {
                listed.name.clone()
            };
            let when = when(listed.written.date_naive(), today);
            (format!("{name:<WHEN_COLUMN$}{when}"), Some(words(listed.words)))
        }
        Item::OpenFolder => {
            let folder = pages.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
            (format!("Open the {folder} folder"), None)
        }
    }
}

/// "Today", "Yesterday", or the weekday within the last week; otherwise the
/// date, with the year if it isn't this one.
fn when(date: NaiveDate, today: NaiveDate) -> String {
    match (today - date).num_days() {
        0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        2..=6 => date.format("%A").to_string(),
        _ if date.year() == today.year() => date.format("%-d %B").to_string(),
        _ => date.format("%-d %B %Y").to_string(),
    }
}

pub fn words(count: usize) -> String {
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

fn open_in_explorer(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_says_the_day_then_the_date() {
        let day = |text| NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap();
        let today = day("2026-11-03");
        assert_eq!(when(today, today), "Today");
        assert_eq!(when(day("2026-11-02"), today), "Yesterday");
        assert_eq!(when(day("2026-10-29"), today), "Thursday");
        assert_eq!(when(day("2026-10-27"), today), "27 October");
        assert_eq!(when(day("2025-12-24"), today), "24 December 2025");
    }
}
