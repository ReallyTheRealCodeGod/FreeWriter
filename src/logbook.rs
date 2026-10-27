use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use chrono::{Datelike, NaiveDate, NaiveTime};

use crate::goal::Goal;

const HEADER: &str = "date\tstart\tgoal\twords\tminutes\tresult";

/// How a session ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Reached,
    /// Left with the panic sentence.
    LeftEarly,
    /// Ended before its goal without the panic sentence: Task Manager, a
    /// crash, the computer turned off.
    Unfinished,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Reached => "reached",
            Outcome::LeftEarly => "left early",
            Outcome::Unfinished => "unfinished",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [Outcome::Reached, Outcome::LeftEarly, Outcome::Unfinished]
            .into_iter()
            .find(|outcome| outcome.as_str() == text)
    }
}

/// One session: a line in the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub date: NaiveDate,
    /// When the session began, to the minute.
    pub start: NaiveTime,
    pub goal: Goal,
    pub words: usize,
    /// Whole minutes on the page.
    pub minutes: u64,
    pub outcome: Outcome,
}

impl Entry {
    fn line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            self.date.format("%Y-%m-%d"),
            self.start.format("%H:%M"),
            self.goal.input(),
            self.words,
            self.minutes,
            self.outcome.as_str(),
        )
    }

    fn parse(line: &str) -> Option<Self> {
        let mut fields = line.trim_end_matches('\r').split('\t');
        let mut next = || fields.next();
        Some(Self {
            date: NaiveDate::parse_from_str(next()?, "%Y-%m-%d").ok()?,
            start: NaiveTime::parse_from_str(next()?, "%H:%M").ok()?,
            goal: Goal::parse(next()?).ok()??,
            words: next()?.parse().ok()?,
            minutes: next()?.parse().ok()?,
            outcome: Outcome::parse(next()?)?,
        })
    }

    /// What's left of the goal, if the session stopped short of it.
    pub fn remaining(&self) -> Option<Goal> {
        match self.goal {
            Goal::Words(n) if n > self.words => Some(Goal::Words(n - self.words)),
            Goal::Time(s) if s > self.minutes * 60 => Some(Goal::Time(s - self.minutes * 60)),
            _ => None,
        }
    }
}

/// The log of sessions, `log.tsv`, and the line of the session that's
/// running, `session.tsv`, both in the app's folder (next to the settings).
///
/// The running line is rewritten as the session goes. Leaving properly moves
/// it into the log; if it's still there at the next start, the session ended
/// some other way, and it's moved over then.
#[derive(Clone)]
pub struct Logbook {
    dir: PathBuf,
}

impl Logbook {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join("log.tsv")
    }

    fn running_path(&self) -> PathBuf {
        self.dir.join("session.tsv")
    }

    /// Every session in the log, oldest first. Lines it can't read are
    /// skipped, so a hand-edited log never stops the app.
    pub fn entries(&self) -> Vec<Entry> {
        let text = fs::read_to_string(self.path()).unwrap_or_default();
        text.lines().filter_map(Entry::parse).collect()
    }

    fn append(&self, entry: &Entry) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.path();
        let new = !path.exists();
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let header = if new { format!("{HEADER}\n") } else { String::new() };
        file.write_all(format!("{header}{}\n", entry.line()).as_bytes())
    }

    /// Records the running session as it stands. Written to a temporary file
    /// and swapped in, like the page.
    pub fn write_running(&self, entry: &Entry) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.running_path();
        let tmp = path.with_extension("tsv.tmp");
        let mut file = File::create(&tmp)?;
        file.write_all(format!("{}\n", entry.line()).as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(tmp, path)
    }

    /// The session has ended properly: into the log it goes.
    pub fn finish(&self, entry: &Entry) -> io::Result<()> {
        self.append(entry)?;
        match fs::remove_file(self.running_path()) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// A session that didn't end properly, if there was one. It's moved into
    /// the log as it stood at its last save.
    pub fn recover(&self) -> Option<Entry> {
        let text = fs::read_to_string(self.running_path()).ok()?;
        let entry = Entry::parse(text.lines().next()?);
        match &entry {
            Some(entry) => self.finish(entry).ok()?,
            None => fs::remove_file(self.running_path()).ok()?,
        }
        entry
    }
}

/// The goal of the last session, to suggest on the start line. A session that
/// finished off an unfinished one had only the rest as its goal, so the goal
/// it finished is the one that counts.
pub fn last_goal(entries: &[Entry]) -> Option<Goal> {
    let mut i = entries.len().checked_sub(1)?;
    while i > 0 {
        let (before, this) = (&entries[i - 1], &entries[i]);
        let finishing = before.date == this.date
            && before.outcome == Outcome::Unfinished
            && before.remaining() == Some(this.goal);
        if !finishing {
            break;
        }
        i -= 1;
    }
    Some(entries[i].goal)
}

/// The quiet line on the start screen: "Yesterday: 523 words · 4 days in a
/// row". `None` before the first session.
pub fn record(entries: &[Entry], today: NaiveDate) -> Option<String> {
    let day = entries.iter().map(|entry| entry.date).max()?;
    let words: usize = entries.iter().filter(|e| e.date == day).map(|e| e.words).sum();
    let words = if words == 1 { "1 word".to_owned() } else { format!("{words} words") };
    let label = if day == today {
        "Today so far".to_owned()
    } else if Some(day) == today.pred_opt() {
        "Yesterday".to_owned()
    } else if day.year() == today.year() {
        format!("Last time, {}", day.format("%-d %B"))
    } else {
        format!("Last time, {}", day.format("%-d %B %Y"))
    };
    match streak(entries, today) {
        n if n >= 2 => Some(format!("{label}: {words} · {n} days in a row")),
        _ => Some(format!("{label}: {words}")),
    }
}

/// Days in a row, ending today or yesterday, on which a goal was reached.
fn streak(entries: &[Entry], today: NaiveDate) -> u32 {
    let reached = |day: NaiveDate| entries.iter().any(|e| e.date == day && e.outcome == Outcome::Reached);
    let mut day = if reached(today) { today } else { today.pred_opt().unwrap_or(today) };
    let mut count = 0;
    while reached(day) {
        count += 1;
        let Some(before) = day.pred_opt() else { break };
        day = before;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn entry(day: &str, goal: Goal, words: usize, minutes: u64, outcome: Outcome) -> Entry {
        let start = NaiveTime::from_hms_opt(9, 14, 0).unwrap();
        Entry { date: date(day), start, goal, words, minutes, outcome }
    }

    fn scratch(name: &str) -> Logbook {
        let dir = std::env::temp_dir().join(format!("writer-log-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Logbook::new(dir)
    }

    #[test]
    fn lines_read_back() {
        let e = entry("2026-10-22", Goal::Time(25 * 60), 310, 25, Outcome::LeftEarly);
        assert_eq!(e.line(), "2026-10-22\t09:14\t25m\t310\t25\tleft early");
        assert_eq!(Entry::parse(&e.line()), Some(e));
        assert_eq!(Entry::parse(HEADER), None);
        assert_eq!(Entry::parse("2026-10-22\t09:14\t500\tlots\t3\treached"), None);
    }

    #[test]
    fn remaining_is_what_is_left_of_the_goal() {
        let words = entry("2026-10-22", Goal::Words(500), 312, 9, Outcome::Unfinished);
        assert_eq!(words.remaining(), Some(Goal::Words(188)));
        let time = entry("2026-10-22", Goal::Time(25 * 60), 0, 13, Outcome::Unfinished);
        assert_eq!(time.remaining(), Some(Goal::Time(12 * 60)));
        let done = entry("2026-10-22", Goal::Words(500), 500, 30, Outcome::Unfinished);
        assert_eq!(done.remaining(), None);
    }

    #[test]
    fn a_running_session_ends_in_the_log_either_way() {
        let log = scratch("running");
        let first = entry("2026-10-21", Goal::Words(500), 523, 27, Outcome::Reached);
        log.write_running(&first).unwrap();
        log.finish(&first).unwrap();
        assert!(log.recover().is_none(), "a finished session leaves nothing behind");

        // Killed mid-session: the next start finds it and logs it.
        let second = entry("2026-10-22", Goal::Words(500), 312, 9, Outcome::Unfinished);
        log.write_running(&second).unwrap();
        assert_eq!(log.recover(), Some(second.clone()));
        assert!(log.recover().is_none());

        assert_eq!(log.entries(), vec![first, second]);
        let text = fs::read_to_string(log.path()).unwrap();
        assert!(text.starts_with(HEADER), "the log has a header for spreadsheets");
        let _ = fs::remove_dir_all(&log.dir);
    }

    #[test]
    fn last_goal_looks_past_finished_off_sessions() {
        assert_eq!(last_goal(&[]), None);
        let mut log = vec![entry("2026-10-21", Goal::Time(25 * 60), 300, 25, Outcome::Reached)];
        assert_eq!(last_goal(&log), Some(Goal::Time(25 * 60)));

        // 500 words, cut at 312, finished off twice: still 500.
        log.push(entry("2026-10-22", Goal::Words(500), 312, 9, Outcome::Unfinished));
        log.push(entry("2026-10-22", Goal::Words(188), 88, 4, Outcome::Unfinished));
        log.push(entry("2026-10-22", Goal::Words(100), 104, 5, Outcome::Reached));
        assert_eq!(last_goal(&log), Some(Goal::Words(500)));

        // A new goal of the same size as the rest, the next day, is its own.
        log.push(entry("2026-10-23", Goal::Words(300), 100, 4, Outcome::Unfinished));
        log.push(entry("2026-10-24", Goal::Words(200), 210, 9, Outcome::Reached));
        assert_eq!(last_goal(&log), Some(Goal::Words(200)));
    }

    #[test]
    fn record_says_what_and_how_many_days() {
        let today = date("2026-10-22");
        assert_eq!(record(&[], today), None);

        let mut log = vec![
            entry("2026-10-19", Goal::Words(500), 520, 30, Outcome::Reached),
            entry("2026-10-20", Goal::Words(500), 100, 5, Outcome::LeftEarly),
            entry("2026-10-20", Goal::Words(500), 510, 25, Outcome::Reached),
            entry("2026-10-21", Goal::Words(500), 523, 27, Outcome::Reached),
        ];
        assert_eq!(record(&log, today).unwrap(), "Yesterday: 523 words · 3 days in a row");

        log.push(entry("2026-10-22", Goal::Words(500), 200, 10, Outcome::Unfinished));
        assert_eq!(record(&log, today).unwrap(), "Today so far: 200 words · 3 days in a row");
        log.push(entry("2026-10-22", Goal::Words(188), 190, 9, Outcome::Reached));
        assert_eq!(record(&log, today).unwrap(), "Today so far: 390 words · 4 days in a row");

        let later = date("2026-11-02");
        assert_eq!(record(&log, later).unwrap(), "Last time, 22 October: 390 words");
        let next_year = date("2027-01-05");
        assert_eq!(record(&log, next_year).unwrap(), "Last time, 22 October 2026: 390 words");
    }
}
