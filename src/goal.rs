use std::time::Duration;

/// What a session has to reach before the page lets you go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Words(usize),
    /// Time spent on the page, in seconds.
    Time(u64),
}

/// Upper limits, so a typo can't lock you in for days.
const MAX_WORDS: usize = 10_000;
const MAX_SECONDS: u64 = 240 * 60;

impl Goal {
    /// Reads the start line: `500` means words, `25m` minutes and `90s`
    /// seconds (handy for trying the lock).
    ///
    /// `Ok(None)` means nothing has been typed yet; `Err` says what's wrong.
    pub fn parse(input: &str) -> Result<Option<Self>, &'static str> {
        let input = input.trim().to_lowercase();
        if input.is_empty() {
            return Ok(None);
        }
        let (number, seconds_each) = if let Some(n) = input.strip_suffix("min").or_else(|| input.strip_suffix('m')) {
            (n, Some(60))
        } else if let Some(n) = input.strip_suffix("sec").or_else(|| input.strip_suffix('s')) {
            (n, Some(1))
        } else {
            (input.as_str(), None)
        };
        let n: u64 = number
            .trim_end()
            .parse()
            .map_err(|_| "Type a number of words, like 500, or minutes, like 25m.")?;
        if n == 0 {
            return Err("The goal has to be more than nothing.");
        }
        match seconds_each {
            Some(each) if n.saturating_mul(each) > MAX_SECONDS => Err("At most 240 minutes."),
            Some(each) => Ok(Some(Goal::Time(n * each))),
            None if n > MAX_WORDS as u64 => Err("At most 10,000 words."),
            None => Ok(Some(Goal::Words(n as usize))),
        }
    }

    /// The goal in words, for the start line.
    pub fn describe(self) -> String {
        match self {
            Goal::Words(1) => "1 word".to_owned(),
            Goal::Words(n) => format!("{n} words"),
            Goal::Time(60) => "1 minute".to_owned(),
            Goal::Time(s) if s % 60 == 0 => format!("{} minutes", s / 60),
            Goal::Time(1) => "1 second".to_owned(),
            Goal::Time(s) => format!("{s} seconds"),
        }
    }

    /// The goal as it would be typed on the start line: `500`, `25m`, `90s`.
    /// `parse` reads it back.
    pub fn input(self) -> String {
        match self {
            Goal::Words(n) => n.to_string(),
            Goal::Time(s) if s % 60 == 0 => format!("{}m", s / 60),
            Goal::Time(s) => format!("{s}s"),
        }
    }

    /// How far along the session is, from 0 to 1.
    pub fn progress(self, words: usize, elapsed: Duration) -> f32 {
        let fraction = match self {
            Goal::Words(n) => words as f32 / n as f32,
            Goal::Time(s) => elapsed.as_secs_f32() / s as f32,
        };
        fraction.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_words_and_m_is_minutes() {
        assert_eq!(Goal::parse("500"), Ok(Some(Goal::Words(500))));
        assert_eq!(Goal::parse(" 25m "), Ok(Some(Goal::Time(25 * 60))));
        assert_eq!(Goal::parse("25 M"), Ok(Some(Goal::Time(25 * 60))));
        assert_eq!(Goal::parse("25min"), Ok(Some(Goal::Time(25 * 60))));
        assert_eq!(Goal::parse("15s"), Ok(Some(Goal::Time(15))));
        assert_eq!(Goal::parse(""), Ok(None));
    }

    #[test]
    fn rejects_nonsense_and_huge_goals() {
        assert!(Goal::parse("abc").is_err());
        assert!(Goal::parse("m").is_err());
        assert!(Goal::parse("0").is_err());
        assert!(Goal::parse("-5").is_err());
        assert!(Goal::parse("10001").is_err());
        assert!(Goal::parse("241m").is_err());
        assert!(Goal::parse("14401s").is_err());
        assert!(Goal::parse("99999999999999999999m").is_err());
        assert_eq!(Goal::parse("10000"), Ok(Some(Goal::Words(10_000))));
        assert_eq!(Goal::parse("240m"), Ok(Some(Goal::Time(240 * 60))));
    }

    #[test]
    fn describes_goals() {
        assert_eq!(Goal::Words(1).describe(), "1 word");
        assert_eq!(Goal::Words(500).describe(), "500 words");
        assert_eq!(Goal::Time(60).describe(), "1 minute");
        assert_eq!(Goal::Time(25 * 60).describe(), "25 minutes");
        assert_eq!(Goal::Time(90).describe(), "90 seconds");
    }

    #[test]
    fn input_reads_back_as_the_same_goal() {
        for goal in [Goal::Words(500), Goal::Time(25 * 60), Goal::Time(90)] {
            assert_eq!(Goal::parse(&goal.input()), Ok(Some(goal)));
        }
        assert_eq!(Goal::Time(25 * 60).input(), "25m");
    }

    #[test]
    fn progress_is_clamped() {
        let words = Goal::Words(100);
        assert_eq!(words.progress(50, Duration::ZERO), 0.5);
        assert_eq!(words.progress(150, Duration::ZERO), 1.0);
        let time = Goal::Time(600);
        assert_eq!(time.progress(0, Duration::from_secs(300)), 0.5);
    }
}
