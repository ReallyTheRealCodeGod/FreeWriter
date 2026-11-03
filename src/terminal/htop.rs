//! htop, as on the first of the other monitors: every core this computer
//! has (WSL sees them all), a light load, and the processes of a small
//! server, refreshed every 1.5 seconds like htop's default. What it shows is
//! worked out from the time, so it needs no state of its own.

use std::time::Duration;

use chrono::{DateTime, Local};

use super::{
    BLACK, BLUE, BRIGHT_BLACK, BRIGHT_WHITE, CYAN, Color32, FOREGROUND, GREEN, Line, Machine, RED, Style, YELLOW,
    noise,
};

/// htop's default refresh.
const DELAY: Duration = Duration::from_millis(1500);
/// What WSL is given by default: half of the computer's memory.
const MEMORY_GB: f64 = 15.5;
const SWAP_GB: f64 = 4.0;

/// A process, as it usually is.
struct Process {
    pid: u32,
    /// `None` is the WSL user.
    user: Option<&'static str>,
    pri: i32,
    ni: i32,
    /// Sizes in KiB.
    virt: u64,
    res: u64,
    shr: u64,
    /// Its usual share of a core, in percent.
    cpu: f64,
    mem: f64,
    command: &'static str,
}

const fn process(
    pid: u32,
    user: Option<&'static str>,
    sizes: (u64, u64, u64),
    cpu: f64,
    mem: f64,
    command: &'static str,
) -> Process {
    Process { pid, user, pri: 20, ni: 0, virt: sizes.0, res: sizes.1, shr: sizes.2, cpu, mem, command }
}

const ROOT: Option<&str> = Some("root");
const POSTGRES: Option<&str> = Some("postgres");
const WWW: Option<&str> = Some("www-data");
const ME: Option<&str> = None;

const PROCESSES: &[Process] = &[
    process(1, ROOT, (22_352, 13_184, 9_472), 0.05, 0.1, "/sbin/init"),
    process(2, ROOT, (2_776, 1_920, 1_792), 0.0, 0.0, "/init"),
    process(7, ROOT, (2_776, 1_920, 1_792), 0.02, 0.0, "plan9 --control-socket 7 --log-level 4 --server-fd 8 --pipe-fd 10 --log-truncate"),
    Process { pri: 19, ni: -1, ..process(54, ROOT, (66_788, 17_664, 16_512), 0.05, 0.1, "/usr/lib/systemd/systemd-journald") },
    process(101, ROOT, (24_120, 6_400, 4_992), 0.0, 0.0, "/usr/lib/systemd/systemd-udevd"),
    process(143, Some("systemd-r"), (21_456, 12_416, 10_368), 0.02, 0.1, "/usr/lib/systemd/systemd-resolved"),
    process(144, Some("systemd-t"), (91_020, 7_552, 6_784), 0.0, 0.0, "/usr/lib/systemd/systemd-timesyncd"),
    process(169, ROOT, (4_236, 2_688, 2_432), 0.0, 0.0, "/usr/sbin/cron -f -P"),
    process(170, Some("messagebu"), (9_628, 5_120, 4_480), 0.02, 0.0, "@dbus-daemon --system --address=systemd: --nofork --nopidfile --systemd-activation --syslog-only"),
    process(174, ROOT, (32_604, 19_456, 10_496), 0.0, 0.1, "/usr/bin/python3 /usr/bin/networkd-dispatcher --run-startup-triggers"),
    process(176, Some("polkitd"), (308_160, 7_936, 6_528), 0.0, 0.0, "/usr/lib/polkit-1/polkitd --no-debug"),
    process(178, ROOT, (17_996, 8_192, 7_168), 0.0, 0.0, "/usr/lib/systemd/systemd-logind"),
    process(181, ROOT, (1_241_420, 14_336, 9_984), 0.05, 0.1, "/usr/libexec/wsl-pro-service -vv"),
    process(182, Some("syslog"), (222_508, 5_376, 4_352), 0.02, 0.0, "/usr/sbin/rsyslogd -n -iNONE"),
    process(191, ROOT, (1_986_312, 31_744, 18_816), 0.1, 0.2, "/usr/lib/snapd/snapd"),
    process(196, ROOT, (6_172, 1_920, 1_792), 0.0, 0.0, "/sbin/agetty -o -p -- \\u --noclear --keep-baud - 115200,38400,9600 vt220"),
    process(203, ROOT, (107_016, 22_272, 13_184), 0.0, 0.1, "/usr/bin/python3 /usr/share/unattended-upgrades/unattended-upgrade-shutdown --wait-for-signal"),
    process(211, ROOT, (2_382_640, 48_896, 32_000), 0.4, 0.3, "/usr/bin/containerd"),
    process(244, ROOT, (2_710_252, 81_408, 50_176), 0.5, 0.5, "/usr/bin/dockerd -H fd:// --containerd=/run/containerd/containerd.sock"),
    process(260, ROOT, (55_204, 1_664, 0), 0.0, 0.0, "nginx: master process /usr/sbin/nginx -g daemon on; master_process on;"),
    process(261, WWW, (55_908, 5_824, 4_096), 0.1, 0.0, "nginx: worker process"),
    process(262, WWW, (55_908, 5_824, 4_096), 0.05, 0.0, "nginx: worker process"),
    process(263, WWW, (55_908, 5_696, 4_096), 0.0, 0.0, "nginx: worker process"),
    process(264, WWW, (55_908, 5_696, 4_096), 0.0, 0.0, "nginx: worker process"),
    process(290, POSTGRES, (222_016, 30_208, 27_520), 0.05, 0.2, "/usr/lib/postgresql/16/bin/postgres -D /var/lib/postgresql/16/main -c config_file=/etc/postgresql/16/main/postgresql.conf"),
    process(293, POSTGRES, (222_152, 9_472, 6_784), 0.0, 0.1, "postgres: 16/main: checkpointer"),
    process(294, POSTGRES, (222_016, 6_656, 3_968), 0.02, 0.0, "postgres: 16/main: background writer"),
    process(296, POSTGRES, (222_016, 10_368, 7_680), 0.02, 0.1, "postgres: 16/main: walwriter"),
    process(297, POSTGRES, (223_600, 8_448, 5_376), 0.0, 0.1, "postgres: 16/main: autovacuum launcher"),
    process(298, POSTGRES, (223_584, 7_168, 4_352), 0.0, 0.0, "postgres: 16/main: logical replication launcher"),
    process(340, ROOT, (1_238_172, 13_312, 9_856), 0.05, 0.1, "/usr/bin/containerd-shim-runc-v2 -namespace moby -id 3f2a9c81d4e7b6a05f1c2e8d9b4a7c3e6f0d1b2a5c8e7f9d0a1b3c4d5e6f7a8b -address /run/containerd/containerd.sock"),
    process(362, Some("472"), (1_317_880, 98_304, 61_440), 0.3, 0.6, "grafana server --homepath=/usr/share/grafana --config=/etc/grafana/grafana.ini --packaging=docker cfg:default.log.mode=console"),
    process(410, ROOT, (2_776, 1_920, 1_792), 0.0, 0.0, "/init"),
    process(411, ROOT, (2_776, 1_920, 1_792), 0.02, 0.0, "/init"),
    process(412, ME, (6_208, 5_248, 3_456), 0.0, 0.0, "-bash"),
    process(430, ME, (20_200, 11_264, 9_216), 0.0, 0.1, "/usr/lib/systemd/systemd --user"),
    process(431, ME, (21_148, 3_520, 1_792), 0.0, 0.0, "(sd-pam)"),
    process(1210, ROOT, (2_780, 1_920, 1_792), 0.0, 0.0, "/init"),
    process(1211, ME, (6_208, 5_120, 3_456), 0.0, 0.0, "-bash"),
    process(1300, ME, (6_208, 5_248, 3_456), 0.0, 0.0, "-bash"),
    process(1388, ME, (6_208, 5_120, 3_456), 0.0, 0.0, "-bash"),
    process(2244, ME, (8_760, 4_992, 3_584), 1.4, 0.0, "htop"),
    process(2310, ME, (23_456, 9_088, 8_064), 0.1, 0.1, "journalctl -f"),
];

pub struct Top {
    machine: Machine,
    seed: u64,
    cpus: usize,
}

impl Top {
    pub fn new(machine: Machine, seed: u64) -> Self {
        let cpus = std::thread::available_parallelism().map_or(8, |n| n.get()).clamp(2, 64);
        Self { machine, seed, cpus }
    }

    /// The screen at `clock`, row by row.
    pub fn screen(&self, cols: usize, rows: usize, clock: Duration, now: DateTime<Local>) -> Vec<Line> {
        let tick = (clock.as_millis() / DELAY.as_millis()) as u64;
        let mut screen = self.header(cols, tick, now);
        screen.push(Line::new());
        screen.push(table_header(cols));
        let room = rows.saturating_sub(screen.len() + 1);
        for (i, row) in self.processes(tick, now).into_iter().take(room).enumerate() {
            let line = row.line(&self.machine.user, cols);
            // The selection bar sits on the top row.
            screen.push(if i == 0 { Line::new().with(line.text_only(), Style::on(BLACK, CYAN)).pad(cols, Style::on(BLACK, CYAN)) } else { line });
        }
        while screen.len() + 1 < rows {
            screen.push(Line::new());
        }
        screen.truncate(rows.saturating_sub(1));
        screen.push(function_bar(cols));
        screen
    }

    /// The meters: cores on both sides, memory under the left ones, tasks,
    /// load and uptime under the right.
    fn header(&self, cols: usize, tick: u64, now: DateTime<Local>) -> Vec<Line> {
        let half = cols / 2;
        let left_cpus = self.cpus / 2;
        let per_column = left_cpus.div_ceil(2).max((self.cpus - left_cpus).div_ceil(2));
        let mut left = Vec::new();
        let mut right = Vec::new();
        for (side, range) in [(&mut left, 0..left_cpus), (&mut right, left_cpus..self.cpus)] {
            let cpus: Vec<usize> = range.collect();
            for row in 0..per_column {
                let mut line = Line::plain(" ");
                // htop fills the columns top to bottom.
                for column in 0..2 {
                    let width = (half - 1) / 2;
                    match cpus.get(column * per_column + row) {
                        Some(&cpu) => line.append(self.cpu_meter(cpu, tick, width)),
                        None => line.push(&" ".repeat(width), Style::PLAIN),
                    }
                }
                side.push(line);
            }
        }
        let wide = half - 2;
        let used = 1.92 + 0.04 * noise(self.seed, &[tick / 4, 1]);
        let (buffers, cache) = (0.09, 1.13 + 0.02 * noise(self.seed, &[tick / 8, 2]));
        left.push(Line::plain(" ").with_line(meter(
            "Mem",
            wide,
            &[(used, GREEN), (buffers, BLUE), (cache, YELLOW)],
            MEMORY_GB,
            &format!("{used:.2}G/{MEMORY_GB:.1}G"),
        )));
        left.push(Line::plain(" ").with_line(meter("Swp", wide, &[(0.0, RED)], SWAP_GB, &format!("0K/{SWAP_GB:.2}G"))));

        let running = 1 + u64::from(noise(self.seed, &[tick, 3]) > 0.7);
        let label = Style::fg(CYAN);
        let value = Style::fg(BRIGHT_WHITE);
        right.push(
            Line::plain(" ")
                .with("Tasks: ", label)
                .with(PROCESSES.len().to_string(), value)
                .with(", ", Style::PLAIN)
                .with((PROCESSES.len() * 4 + 3).to_string(), value)
                .with(" thr, ", Style::PLAIN)
                .with("0", value)
                .with(" kthr; ", Style::PLAIN)
                .with(running.to_string(), Style::fg(GREEN))
                .with(" running", Style::PLAIN),
        );
        let (one, five, fifteen) = self.load(tick);
        right.push(
            Line::plain(" ")
                .with("Load average: ", label)
                .with(format!("{one:.2} "), value)
                .with(format!("{five:.2} "), Style::PLAIN)
                .with(format!("{fifteen:.2} "), Style::fg(BRIGHT_BLACK)),
        );
        let up = now - self.machine.booted;
        let uptime = format!(
            "{} days, {:02}:{:02}:{:02}",
            up.num_days(),
            up.num_hours() % 24,
            up.num_minutes() % 60,
            up.num_seconds() % 60
        );
        right.push(Line::plain(" ").with("Uptime: ", label).with(uptime, value));

        (0..left.len().max(right.len()))
            .map(|i| {
                let mut line = left.get(i).cloned().unwrap_or_default().truncate(half).pad(half, Style::PLAIN);
                line.append(right.get(i).cloned().unwrap_or_default());
                line.truncate(cols)
            })
            .collect()
    }

    fn cpu_meter(&self, cpu: usize, tick: u64, width: usize) -> Line {
        let n = noise(self.seed, &[tick, cpu as u64, 10]);
        let mut busy = 0.6 + 7.0 * n * n;
        if noise(self.seed, &[tick, cpu as u64, 11]) > 0.94 {
            busy += 15.0 + 45.0 * noise(self.seed, &[tick, cpu as u64, 12]);
        }
        let kernel = busy * (0.15 + 0.25 * noise(self.seed, &[tick, cpu as u64, 13]));
        let low = if cpu % 5 == 0 { 0.4 } else { 0.0 };
        meter(
            &format!("{cpu:>3}"),
            width.saturating_sub(1),
            &[(low, BLUE), (busy - kernel, GREEN), (kernel, RED)],
            100.0,
            &format!("{busy:.1}%"),
        )
        .with(" ", Style::PLAIN)
    }

    fn load(&self, tick: u64) -> (f64, f64, f64) {
        let minutes = tick as f64 * DELAY.as_secs_f64() / 60.0;
        let one = 0.42 + 0.3 * (minutes / 1.7).sin().abs() + 0.08 * noise(self.seed, &[tick / 3, 20]);
        let five = 0.48 + 0.12 * (minutes / 9.0).sin().abs();
        let fifteen = 0.51 + 0.05 * (minutes / 30.0).sin().abs();
        (one, five, fifteen)
    }

    /// The processes at `tick`, busiest first.
    fn processes(&self, tick: u64, now: DateTime<Local>) -> Vec<Row> {
        let up = (now - self.machine.booted).num_seconds().max(0) as f64;
        let mut rows: Vec<Row> = PROCESSES
            .iter()
            .map(|process| {
                let n = noise(self.seed, &[tick, u64::from(process.pid), 30]);
                let mut cpu = process.cpu * (0.2 + 1.6 * n);
                if process.cpu > 0.0 && noise(self.seed, &[tick, u64::from(process.pid), 31]) > 0.97 {
                    cpu += 2.0 + 12.0 * n;
                }
                // Too little to show rounds to nothing, as in htop.
                let cpu = (cpu * 10.0).round() / 10.0;
                let time = up * process.cpu / 100.0 * 0.4 + f64::from(process.pid % 97) / 10.0;
                Row { process, cpu, time }
            })
            .collect();
        rows.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(a.process.pid.cmp(&b.process.pid)));
        rows
    }
}

struct Row {
    process: &'static Process,
    cpu: f64,
    /// TIME+, in seconds.
    time: f64,
}

impl Row {
    fn line(&self, user: &str, cols: usize) -> Line {
        let p = self.process;
        let name = p.user.unwrap_or(user);
        let state = if p.command == "htop" || self.cpu >= 5.0 { "R" } else { "S" };
        let mut line = Line::plain(format!("{:>7} {:<9} {:>3} {:>3} ", p.pid, truncated(name, 9), p.pri, p.ni));
        for kib in [p.virt, p.res, p.shr] {
            let (text, style) = memory(kib);
            line.push(&format!("{text:>5}"), style);
            line.push(" ", Style::PLAIN);
        }
        line.push(state, if state == "R" { Style::fg(GREEN) } else { Style::PLAIN });
        line.push(&format!(" {:>5.1} {:>4.1} ", self.cpu, p.mem), Style::PLAIN);
        let (time, hours) = time_plus(self.time);
        line.push(&format!("{:>width$}", "", width = 9 - time.chars().count() - hours.chars().count()), Style::PLAIN);
        line.push(&hours, Style::fg(CYAN));
        line.push(&time, Style::PLAIN);
        line.push("  ", Style::PLAIN);
        line.push(p.command, Style::PLAIN);
        line.truncate(cols)
    }
}

impl Line {
    fn with_line(mut self, other: Line) -> Self {
        self.append(other);
        self
    }

    /// Just the characters, for a row drawn all in one colour.
    fn text_only(&self) -> String {
        self.spans.iter().map(|(text, _)| text.as_str()).collect()
    }
}

/// A bar meter: `Mem[|||||||       1.92G/15.5G]`, the value in the bar's
/// right end, over the bars where they reach it.
fn meter(caption: &str, width: usize, parts: &[(f64, Color32)], total: f64, text: &str) -> Line {
    let inner = width.saturating_sub(caption.chars().count() + 2);
    let mut colours = Vec::with_capacity(inner);
    let mut sum = 0.0;
    for (amount, colour) in parts {
        sum += amount;
        let reach = ((sum / total).clamp(0.0, 1.0) * inner as f64).round() as usize;
        while colours.len() < reach {
            colours.push(*colour);
        }
    }
    let text: Vec<char> = text.chars().collect();
    let text_start = inner.saturating_sub(text.len());
    let mut line = Line::new().with(caption, Style::fg(CYAN)).with("[", Style::fg(BRIGHT_WHITE));
    for i in 0..inner {
        let c = if i >= text_start { text[i - text_start] } else if i < colours.len() { '|' } else { ' ' };
        let style = colours.get(i).map_or(Style::fg(BRIGHT_BLACK), |colour| Style::fg(*colour));
        line.push(c.encode_utf8(&mut [0; 4]), style);
    }
    line.with("]", Style::fg(BRIGHT_WHITE))
}

fn table_header(cols: usize) -> Line {
    let header = Style::on(BLACK, GREEN);
    Line::new()
        .with(format!("{:>7} {:<9} {:>3} {:>3} {:>5} {:>5} {:>5} S", "PID", "USER", "PRI", "NI", "VIRT", "RES", "SHR"), header)
        .with(" CPU%▽", Style::on(BLACK, CYAN))
        .with(format!("MEM% {:>9}  Command", "TIME+"), header)
        .pad(cols, header)
        .truncate(cols)
}

/// `F1Help  F2Setup ...` along the bottom.
fn function_bar(cols: usize) -> Line {
    let mut line = Line::new();
    for (key, label) in [
        ("F1", "Help  "),
        ("F2", "Setup "),
        ("F3", "Search"),
        ("F4", "Filter"),
        ("F5", "Tree  "),
        ("F6", "SortBy"),
        ("F7", "Nice -"),
        ("F8", "Nice +"),
        ("F9", "Kill  "),
        ("F10", "Quit  "),
    ] {
        line.push(key, Style::fg(FOREGROUND));
        line.push(label, Style::on(BLACK, CYAN));
    }
    line.pad(cols, Style::on(BLACK, CYAN)).truncate(cols)
}

/// Memory as htop writes it: KiB up to 99999, then M, then G.
fn memory(kib: u64) -> (String, Style) {
    if kib < 100_000 {
        (kib.to_string(), Style::PLAIN)
    } else if kib < 10_000 * 1024 {
        (format!("{}M", kib / 1024), Style::fg(CYAN))
    } else {
        (format!("{:.1}G", kib as f64 / 1024.0 / 1024.0), Style::fg(GREEN))
    }
}

/// TIME+: `0:07.42` under an hour, `1h05:22` past it, with the hours apart
/// as htop colours them.
fn time_plus(seconds: f64) -> (String, String) {
    let hundredths = (seconds * 100.0) as u64;
    let (minutes, rest) = (hundredths / 6000, hundredths % 6000);
    if minutes < 60 {
        (format!("{minutes}:{:02}.{:02}", rest / 100, rest % 100), String::new())
    } else {
        (format!("{:02}:{:02}", minutes % 60, rest / 100), format!("{}h", minutes / 60))
    }
}

fn truncated(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top() -> Top {
        let booted = Local::now() - chrono::Duration::hours(76);
        Top::new(Machine { user: "peter".to_owned(), host: "DESKTOP-4KQ2".to_owned(), booted }, 3)
    }

    #[test]
    fn the_screen_fits_and_has_htops_parts() {
        let top = top();
        for (cols, rows) in [(293, 70), (120, 40), (80, 24)] {
            let screen = top.screen(cols, rows, Duration::from_secs(90), Local::now());
            assert_eq!(screen.len(), rows);
            assert!(screen.iter().all(|line| line.len() <= cols), "{cols}x{rows}");
            let text: Vec<String> = screen.iter().map(Line::text).collect();
            assert!(text.iter().any(|line| line.contains("Mem[")));
            assert!(text.iter().any(|line| line.contains("Load average: ")));
            assert!(text.iter().any(|line| line.contains("PID USER")));
            assert!(text.last().unwrap().starts_with("F1Help  F2Setup F3SearchF4FilterF5Tree"));
        }
    }

    #[test]
    fn it_changes_every_refresh_and_holds_between() {
        let top = top();
        let now = Local::now();
        let at = |ms| top.screen(200, 50, Duration::from_millis(ms), now);
        assert_eq!(at(3_000), at(4_400), "the same refresh");
        assert_ne!(at(3_000), at(4_600), "the next one");
    }

    #[test]
    fn numbers_read_like_htop() {
        assert_eq!(memory(61_428).0, "61428");
        assert_eq!(memory(2_382_640).0, "2326M");
        assert_eq!(memory(12_000_000).0, "11.4G");
        assert_eq!(time_plus(7.42), ("0:07.42".to_owned(), String::new()));
        assert_eq!(time_plus(3922.0), ("05:22".to_owned(), "1h".to_owned()));
    }
}
