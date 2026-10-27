use std::fs;
use std::path::PathBuf;

/// Settings from `%APPDATA%\writer\settings.toml`. There's no settings screen;
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
    Some(dirs::config_dir()?.join("writer"))
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
}
