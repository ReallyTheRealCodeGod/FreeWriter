use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

    /// Writes the page if anything changed. Writes to a temporary file first
    /// and then swaps it in, so a crash mid-save can't leave half a page.
    pub fn save(&mut self) -> io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("txt.tmp");
        let mut file = File::create(&tmp)?;
        file.write_all(self.text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, &self.path)?;
        self.dirty = false;
        Ok(())
    }
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
