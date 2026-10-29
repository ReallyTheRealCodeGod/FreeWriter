use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::NaiveDate;

/// The day's page: one plain text file per day in `Documents\Writing`.
///
/// Text is only ever added at the end. Backspace can take back what was typed
/// in this session, but never touches what was already on the page.
pub struct Page {
    path: PathBuf,
    text: String,
    /// Byte index where this session's writing begins.
    session_start: usize,
    dirty: bool,
}

/// The folder the pages go in: `Documents\Writing`.
pub fn pages_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_default()
        .join("Writing")
}

/// Today's page in that folder, e.g. `2026-10-21.txt`.
pub fn today_file(pages: &Path) -> PathBuf {
    let date = chrono::Local::now().format("%Y-%m-%d");
    pages.join(format!("{date}.txt"))
}

pub fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

impl Page {
    /// Reads the page, ready for a new session at the end of it.
    ///
    /// Fails if the file exists but can't be read, so we never overwrite it.
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let mut text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        // A new session starts on a fresh paragraph. This isn't saved unless
        // something gets typed.
        if !text.is_empty() && !text.ends_with("\n\n") {
            text.push_str(if text.ends_with('\n') { "\n" } else { "\n\n" });
        }
        let session_start = text.len();
        Ok(Self { path, text, session_start, dirty: false })
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn type_text(&mut self, typed: &str) {
        let before = self.text.len();
        self.text.extend(typed.chars().filter(|c| !c.is_control()));
        self.dirty |= self.text.len() != before;
    }

    pub fn new_line(&mut self) {
        self.text.push('\n');
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.text.len() > self.session_start {
            self.text.pop();
            self.dirty = true;
        }
    }

    /// Words written in this session.
    pub fn session_words(&self) -> usize {
        count_words(&self.text[self.session_start..])
    }

    /// The end of the text, enough to fill the screen above the typing line
    /// without laying out the whole day on every keystroke. Starts at a line
    /// break where possible so wrapping doesn't shift as you type.
    pub fn tail(&self, max_bytes: usize) -> &str {
        let text = &self.text;
        if text.len() <= max_bytes {
            return text;
        }
        let mut start = text.len() - max_bytes;
        while !text.is_char_boundary(start) {
            start += 1;
        }
        match text[start..].find('\n') {
            Some(i) if i < max_bytes / 2 => &text[start + i + 1..],
            _ => &text[start..],
        }
    }

    /// Writes the page if anything changed.
    pub fn save(&mut self) -> io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        write_safely(&self.path, self.text.as_bytes())?;
        self.dirty = false;
        Ok(())
    }
}

/// Writes to a temporary file first and then swaps it in, so a crash
/// mid-save can't leave half a file.
fn write_safely(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("txt.tmp");
    let mut file = File::create(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

/// A page or a saved copy, as the menu lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub path: PathBuf,
    pub title: Title,
    pub words: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Title {
    /// A day's page, `2026-10-22.txt`.
    Day(NaiveDate),
    /// A copy saved under a name, `Saved\Short story.txt`.
    Copy(String),
}

/// Where saved copies go: a `Saved` folder next to the pages.
pub fn copies_dir(pages: &Path) -> PathBuf {
    pages.join("Saved")
}

/// The day pages, newest first. Other files in the folder are left out.
pub fn day_pages(pages: &Path) -> Vec<Listed> {
    let mut list: Vec<(NaiveDate, PathBuf)> = text_files(pages)
        .filter_map(|(path, name)| Some((NaiveDate::parse_from_str(&name, "%Y-%m-%d").ok()?, path)))
        .collect();
    list.sort_by(|a, b| b.0.cmp(&a.0));
    list.into_iter()
        .map(|(date, path)| Listed { words: count_words(&read_text(&path)), path, title: Title::Day(date) })
        .collect()
}

/// The saved copies, the most recently saved first.
pub fn saved_copies(pages: &Path) -> Vec<Listed> {
    let mut list: Vec<(SystemTime, Listed)> = text_files(&copies_dir(pages))
        .map(|(path, name)| {
            let saved = fs::metadata(&path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            (saved, Listed { words: count_words(&read_text(&path)), path, title: Title::Copy(name) })
        })
        .collect();
    list.sort_by(|a, b| b.0.cmp(&a.0));
    list.into_iter().map(|(_, listed)| listed).collect()
}

/// The `.txt` files in a folder, with their names minus the extension.
fn text_files(dir: &Path) -> impl Iterator<Item = (PathBuf, String)> {
    fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|entry| {
        let path = entry.path();
        let is_text = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("txt"));
        let name = path.file_stem()?.to_str()?.to_owned();
        (is_text && path.is_file()).then_some((path, name))
    })
}

/// A page's text for reading. A file that isn't valid UTF-8 (edited
/// elsewhere, say) still shows, with odd characters marked.
pub fn read_text(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    String::from_utf8_lossy(&bytes).replace('\r', "")
}

/// Whether a character can be part of a file name on Windows.
pub fn allowed_in_name(c: char) -> bool {
    !c.is_control() && !r#"\/:*?"<>|"#.contains(c)
}

/// The file a copy called `name` is saved as, or what's wrong with the name.
pub fn copy_path(pages: &Path, name: &str) -> Result<PathBuf, &'static str> {
    // Windows drops spaces and dots from the end of a file name.
    let name = name.trim().trim_end_matches(['.', ' ']);
    if name.is_empty() {
        return Err("Type a name for the copy.");
    }
    if !name.chars().all(allowed_in_name) {
        return Err(r#"A name can't have \ / : * ? " < > or | in it."#);
    }
    let base = name.split('.').next().unwrap_or(name).trim_end().to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&base.as_str())
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && base.ends_with(|c: char| ('1'..='9').contains(&c)));
    if reserved {
        return Err("Windows keeps that name for itself. Try another.");
    }
    Ok(copies_dir(pages).join(format!("{name}.txt")))
}

/// Saves a copy of a page, byte for byte, written as safely as the pages.
pub fn save_copy(from: &Path, to: &Path) -> io::Result<()> {
    write_safely(to, &fs::read(from)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("writer-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn new_page_saves_what_was_typed() {
        let path = scratch_dir("new").join("Writing").join("day.txt");
        let mut page = Page::open(path.clone()).unwrap();
        page.type_text("Hello  world");
        page.new_line();
        page.type_text("again");
        assert_eq!(page.session_words(), 3);
        page.save().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "Hello  world\nagain");
        assert!(!page.is_dirty());
    }

    #[test]
    fn continuing_starts_a_new_paragraph_and_protects_old_text() {
        let dir = scratch_dir("continue");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("day.txt");
        fs::write(&path, "Earlier words").unwrap();

        let mut page = Page::open(path.clone()).unwrap();
        assert_eq!(page.session_words(), 0);
        page.type_text("New");
        for _ in 0..20 {
            page.backspace();
        }
        page.type_text("Fresh start");
        assert_eq!(page.session_words(), 2);
        page.save().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "Earlier words\n\nFresh start");
    }

    #[test]
    fn opening_without_typing_writes_nothing() {
        let dir = scratch_dir("untouched");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("day.txt");
        fs::write(&path, "Earlier\n").unwrap();
        let mut page = Page::open(path.clone()).unwrap();
        page.save().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "Earlier\n");

        let missing = dir.join("missing.txt");
        Page::open(missing.clone()).unwrap().save().unwrap();
        assert!(!missing.exists());
    }

    #[test]
    fn unreadable_page_is_not_opened() {
        let dir = scratch_dir("unreadable");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("day.txt");
        fs::write(&path, [0x66, 0xe6, 0x72]).unwrap(); // "fær" in Windows-1252, not UTF-8
        assert!(Page::open(path).is_err());
    }

    #[test]
    fn control_characters_are_ignored() {
        let mut page = Page::open(scratch_dir("control").join("day.txt")).unwrap();
        page.type_text("a\tb\u{8}c");
        assert_eq!(page.tail(100), "abc");
    }

    #[test]
    fn pages_are_listed_newest_first_and_copies_apart() {
        let dir = scratch_dir("listing");
        fs::create_dir_all(copies_dir(&dir)).unwrap();
        fs::write(dir.join("2026-10-20.txt"), "Three words here").unwrap();
        fs::write(dir.join("2026-10-22.txt"), "One").unwrap();
        fs::write(dir.join("notes.txt"), "not a page").unwrap();
        fs::write(dir.join("2026-10-21.txt.tmp"), "half a save").unwrap();
        fs::write(copies_dir(&dir).join("Short story.txt"), "Once upon a time").unwrap();

        let days = day_pages(&dir);
        let dates: Vec<_> = days.iter().map(|l| l.title.clone()).collect();
        let day = |d| Title::Day(NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap());
        assert_eq!(dates, vec![day("2026-10-22"), day("2026-10-20")]);
        assert_eq!((days[0].words, days[1].words), (1, 3));

        let copies = saved_copies(&dir);
        assert_eq!(copies.len(), 1);
        assert_eq!((&copies[0].title, copies[0].words), (&Title::Copy("Short story".to_owned()), 4));
        assert!(day_pages(&dir.join("missing")).is_empty());
    }

    #[test]
    fn copy_names_follow_windows_rules() {
        let dir = Path::new("pages");
        assert_eq!(copy_path(dir, " Short story. "), Ok(dir.join("Saved").join("Short story.txt")));
        assert_eq!(copy_path(dir, "Brev til Åse"), Ok(dir.join("Saved").join("Brev til Åse.txt")));
        assert!(copy_path(dir, "   ").is_err());
        assert!(copy_path(dir, "a/b").is_err());
        for reserved in ["con", "NUL", "com1", "Lpt9.txt", "aux "] {
            assert!(copy_path(dir, reserved).is_err(), "{reserved}");
        }
        assert!(copy_path(dir, "Console").is_ok());
        assert!(copy_path(dir, "COM10").is_ok());
        assert!(!allowed_in_name('?') && allowed_in_name('æ'));
    }

    #[test]
    fn a_copy_is_the_page_byte_for_byte() {
        let dir = scratch_dir("copy");
        fs::create_dir_all(&dir).unwrap();
        let page = dir.join("2026-10-22.txt");
        fs::write(&page, "Æble\nline two").unwrap();
        let copy = copy_path(&dir, "Æble").unwrap();
        save_copy(&page, &copy).unwrap();
        assert_eq!(fs::read(&copy).unwrap(), fs::read(&page).unwrap());
        assert_eq!(read_text(&copy), "Æble\nline two");
    }

    #[test]
    fn tail_starts_at_a_line_break() {
        let mut page = Page::open(scratch_dir("tail").join("day.txt")).unwrap();
        page.type_text("æøå first line");
        page.new_line();
        page.type_text("second line");
        assert_eq!(page.tail(1000), "æøå first line\nsecond line");
        assert_eq!(page.tail(14), "second line");
        assert_eq!(page.tail(3), "ine");
    }
}
