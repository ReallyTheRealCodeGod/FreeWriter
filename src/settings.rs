use std::fs;
use std::path::PathBuf;

/// Settings from `%APPDATA%\writer\settings.toml`. There's no settings screen;
/// the file is edited by hand. Currently it understands one line:
///
/// ```toml
/// panic_exit = true
/// ```
#[derive(Default)]
pub struct Settings {
    /// Allows leaving a session early by typing a long sentence.
    pub panic_exit: bool,
}

pub fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("writer").join("settings.toml"))
}

impl Settings {
    pub fn load() -> Self {
        path()
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    fn parse(text: &str) -> Self {
        let panic_exit = text
            .lines()
            .filter_map(|line| line.split('#').next()?.split_once('='))
            .any(|(key, value)| key.trim() == "panic_exit" && value.trim() == "true");
        Self { panic_exit }
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
}
