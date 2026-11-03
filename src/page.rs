use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime};

/// A page: one plain text file in `Documents\Writing`, new and blank or
/// written on before.
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

/// Where an earlier version saved named copies. They're listed with the
/// pages; new names go in the pages folder itself.
const OLD_COPIES: &str = "Saved";

/// The file for a new page until it's named: the time it was begun, e.g.
/// `2026-11-03 14.05.txt`, with a number after it if that's taken.
pub fn new_file(pages: &Path, now: DateTime<Local>) -> PathBuf {
    let stem = now.format("%Y-%m-%d %H.%M").to_string();
    let mut path = pages.join(format!("{stem}.txt"));
    let mut n = 2;
    while path.exists() {
        path = pages.join(format!("{stem} {n}.txt"));
        n += 1;
    }
    path
}

/// Whether a page has only a date for a name: a new page not named yet, or a
/// day's page from before pages had names.
pub fn is_unnamed(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else { return false };
    // "2026-11-03 14.05 2", the second new page begun that minute.
    let stem = match stem.rsplit_once(' ') {
        Some((rest, n)) if n.parse::<u32>().is_ok() => rest,
        _ => stem,
    };
    NaiveDateTime::parse_from_str(stem, "%Y-%m-%d %H.%M").is_ok() || NaiveDate::parse_from_str(stem, "%Y-%m-%d").is_ok()
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

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Whether there's nothing on the page at all, from before or now.
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
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
    /// without laying out the whole page on every keystroke. Starts at a line
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

    /// Gives the page a new file name: the file moves to `to`, and it's saved
    /// there from now on. Never replaces a file that's already there.
    pub fn rename(&mut self, to: PathBuf) -> io::Result<()> {
        if to.exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "there's a page by that name already"));
        }
        match fs::rename(&self.path, &to) {
            Ok(()) => {}
            // Nothing saved yet. Named, it's saved even if it's empty, so it
            // shows up in the menu.
            Err(e) if e.kind() == io::ErrorKind::NotFound => self.dirty = true,
            Err(e) => return Err(e),
        }
        self.path = to;
        self.save()
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

/// A page as the menu lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub path: PathBuf,
    /// The file's name without `.txt`.
    pub name: String,
    /// When it was last written to.
    pub written: DateTime<Local>,
    pub words: usize,
}

impl Listed {
    pub fn of(path: PathBuf) -> Self {
        let name = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
        let written = fs::metadata(&path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        Self { words: count_words(&read_text(&path)), written: written.into(), name, path }
    }
}

/// Every page, the one written to last first. Other files are left out.
pub fn all_pages(pages: &Path) -> Vec<Listed> {
    let mut list: Vec<Listed> = text_files(pages)
        .chain(text_files(&pages.join(OLD_COPIES)))
        .map(Listed::of)
        .collect();
    list.sort_by(|a, b| b.written.cmp(&a.written).then_with(|| a.name.cmp(&b.name)));
    list
}

/// The `.txt` files in a folder.
fn text_files(dir: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(dir).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| {
        let is_text = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("txt"));
        is_text && path.is_file()
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

/// The file a page called `name` is saved as, or what's wrong with the name.
pub fn named_file(pages: &Path, name: &str) -> Result<PathBuf, &'static str> {
    // Windows drops spaces and dots from the end of a file name.
    let name = name.trim().trim_end_matches(['.', ' ']);
    if name.is_empty() {
        return Err("Type a name for the page.");
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
    Ok(pages.join(format!("{name}.txt")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

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
    fn new_pages_are_named_by_the_minute_they_begin() {
        let dir = scratch_dir("newfile");
        fs::create_dir_all(&dir).unwrap();
        let now = Local.with_ymd_and_hms(2026, 11, 3, 14, 5, 59).unwrap();
        let first = new_file(&dir, now);
        assert_eq!(first, dir.join("2026-11-03 14.05.txt"));
        assert_eq!(new_file(&dir, now), first, "nothing saved there yet");
        fs::write(&first, "taken").unwrap();
        assert_eq!(new_file(&dir, now), dir.join("2026-11-03 14.05 2.txt"));

        for unnamed in ["2026-11-03 14.05.txt", "2026-11-03 14.05 2.txt", "2026-10-29.txt"] {
            assert!(is_unnamed(Path::new(unnamed)), "{unnamed}");
        }
        for named in ["Short story.txt", "Brev 2.txt", "2026.txt", "Den 3. november 14.05.txt"] {
            assert!(!is_unnamed(Path::new(named)), "{named}");
        }
    }

    #[test]
    fn naming_moves_the_page_and_never_replaces_another() {
        let dir = scratch_dir("rename");
        fs::create_dir_all(&dir).unwrap();
        let mut page = Page::open(dir.join("2026-11-03 14.05.txt")).unwrap();
        page.type_text("Once");
        page.save().unwrap();
        page.type_text(" upon");
        page.rename(dir.join("Short story.txt")).unwrap();
        assert!(!dir.join("2026-11-03 14.05.txt").exists(), "moved, not copied");
        assert_eq!(fs::read_to_string(dir.join("Short story.txt")).unwrap(), "Once upon");

        fs::write(dir.join("Taken.txt"), "Someone else's").unwrap();
        assert!(page.rename(dir.join("Taken.txt")).is_err());
        assert_eq!(fs::read_to_string(dir.join("Taken.txt")).unwrap(), "Someone else's");
        assert_eq!(page.path(), dir.join("Short story.txt"));

        // Named before anything was typed: it's saved, empty, under the name.
        let mut blank = Page::open(dir.join("2026-11-03 15.00.txt")).unwrap();
        blank.rename(dir.join("Later.txt")).unwrap();
        assert_eq!(fs::read_to_string(dir.join("Later.txt")).unwrap(), "");
    }

    #[test]
    fn pages_are_listed_last_written_first() {
        let dir = scratch_dir("listing");
        fs::create_dir_all(dir.join(OLD_COPIES)).unwrap();
        let write = |path: PathBuf, text: &str, days_ago: u64| {
            fs::write(&path, text).unwrap();
            let when = SystemTime::now() - std::time::Duration::from_secs(days_ago * 86_400);
            File::options().write(true).open(&path).unwrap().set_modified(when).unwrap();
        };
        write(dir.join("2026-10-20.txt"), "Three words here", 9);
        write(dir.join("Short story.txt"), "One", 0);
        write(dir.join(OLD_COPIES).join("Old copy.txt"), "Once upon a time", 3);
        fs::write(dir.join("2026-10-21.txt.tmp"), "half a save").unwrap();
        fs::write(dir.join("notes.md"), "not a page").unwrap();

        let list = all_pages(&dir);
        let names: Vec<(&str, usize)> = list.iter().map(|l| (l.name.as_str(), l.words)).collect();
        assert_eq!(names, vec![("Short story", 1), ("Old copy", 4), ("2026-10-20", 3)]);
        assert!(all_pages(&dir.join("missing")).is_empty());
    }

    #[test]
    fn names_follow_windows_rules() {
        let dir = Path::new("pages");
        assert_eq!(named_file(dir, " Short story. "), Ok(dir.join("Short story.txt")));
        assert_eq!(named_file(dir, "Brev til Åse"), Ok(dir.join("Brev til Åse.txt")));
        assert!(named_file(dir, "   ").is_err());
        assert!(named_file(dir, "a/b").is_err());
        for reserved in ["con", "NUL", "com1", "Lpt9.txt", "aux "] {
            assert!(named_file(dir, reserved).is_err(), "{reserved}");
        }
        assert!(named_file(dir, "Console").is_ok());
        assert!(named_file(dir, "COM10").is_ok());
        assert!(!allowed_in_name('?') && allowed_in_name('æ'));
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
