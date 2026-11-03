use std::fs;
use std::path::{Path, PathBuf};

/// Settings from `%APPDATA%\FreeWriter\settings.toml`. There's no settings screen;
/// the file is edited by hand. It understands two lines:
///
/// ```toml
/// panic_exit = true   # Esc can end a session early, with a long sentence
/// backspace = false   # a pure typewriter: no erasing on the page
/// ```
pub struct Settings {
    /// Allows leaving a session early by typing a long sentence.
    pub panic_exit: bool,
    /// Whether Backspace can take back letters typed this session.
    pub backspace: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { panic_exit: false, backspace: true }
    }
}

/// The folder the settings, the log and the running session live in.
pub fn dir() -> Option<PathBuf> {
    dirs::config_dir().map(|config| moved_in(&config.join("writer"), config.join("FreeWriter")))
}

/// `dir`, after moving `old` there if only `old` exists: the app was called
/// Writer before. If the move fails, `old` stays in use.
fn moved_in(old: &Path, dir: PathBuf) -> PathBuf {
    if !dir.exists() && old.is_dir() && fs::rename(old, &dir).is_err() {
        return old.to_owned();
    }
    dir
}

impl Settings {
    pub fn load() -> Self {
        dir()
            .and_then(|dir| fs::read_to_string(dir.join("settings.toml")).ok())
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        let lines = text.lines().filter_map(|line| line.split('#').next()?.split_once('='));
        for (key, value) in lines {
            let value = match value.trim() {
                "true" => true,
                "false" => false,
                _ => continue,
            };
            match key.trim() {
                "panic_exit" => settings.panic_exit = value,
                "backspace" => settings.backspace = value,
                _ => {}
            }
        }
        settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_panic_exit() {
        assert!(Settings::parse("panic_exit = true").panic_exit);
        assert!(Settings::parse("# notes\npanic_exit=true # yes\n").panic_exit);
        assert!(!Settings::parse("panic_exit = false").panic_exit);
        assert!(!Settings::parse("# panic_exit = true").panic_exit);
        assert!(!Settings::parse("").panic_exit);
    }

    #[test]
    fn backspace_is_on_unless_turned_off() {
        assert!(Settings::parse("").backspace);
        assert!(Settings::parse("backspace = maybe").backspace);
        assert!(!Settings::parse("backspace = false").backspace);
        let both = Settings::parse("panic_exit = true\nbackspace = false\n");
        assert!(both.panic_exit && !both.backspace);
    }

    #[test]
    fn the_old_folder_is_moved_once() {
        let config = std::env::temp_dir().join(format!("writer-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&config);
        let (old, dir) = (config.join("writer"), config.join("FreeWriter"));
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("log.tsv"), "the log").unwrap();

        assert_eq!(moved_in(&old, dir.clone()), dir);
        assert_eq!(fs::read_to_string(dir.join("log.tsv")).unwrap(), "the log");
        assert!(!old.exists());

        // An old folder turning up later is left alone.
        fs::create_dir_all(&old).unwrap();
        assert_eq!(moved_in(&old, dir.clone()), dir);
        assert!(old.exists());
        let _ = fs::remove_dir_all(&config);
    }
}
