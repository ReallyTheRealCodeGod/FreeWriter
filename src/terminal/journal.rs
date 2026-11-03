//! `journalctl -f`, as on the second of the other monitors: the system log of
//! a quiet server going by, stamped with the real time.

use chrono::{DateTime, Duration, Local, Timelike};

use super::{BRIGHT_RED, Cx, Line, Plan, Rng, Style, WARNING};

pub struct Journal {
    began: bool,
    next_pid: u64,
    session: u64,
}

impl Journal {
    pub fn new() -> Self {
        Self { began: false, next_pid: 5_204, session: 3 }
    }

    fn pid(&mut self, rng: &mut Rng) -> u64 {
        self.next_pid += rng.range(1, 40);
        self.next_pid
    }

    /// Something that happens on a server now and then, as log lines. Most
    /// are routine; a few are warnings.
    fn event(&mut self, rng: &mut Rng, user: &str, time: DateTime<Local>) -> Vec<(String, Style)> {
        let plain = |text: String| (text, Style::PLAIN);
        match rng.range(0, 100) {
            0..=17 => {
                let pid = self.pid(rng);
                let job = if time.minute() == 17 {
                    "   cd / && run-parts --report /etc/cron.hourly"
                } else {
                    rng.pick(&[
                        "command -v debian-sa1 > /dev/null && debian-sa1 1 1",
                        "[ -x /usr/lib/php/sessionclean ] && if [ ! -d /run/systemd/system ]; then /usr/lib/php/sessionclean; fi",
                        "test -x /usr/sbin/anacron || { cd / && run-parts --report /etc/cron.daily; }",
                    ])
                };
                vec![
                    plain(format!("CRON[{pid}]: pam_unix(cron:session): session opened for user root(uid=0) by root(uid=0)")),
                    plain(format!("CRON[{}]: (root) CMD ({job})", pid + 1)),
                    plain(format!("CRON[{pid}]: pam_unix(cron:session): session closed for user root")),
                ]
            }
            18..=29 => {
                let id = container(rng);
                vec![
                    plain(format!("dockerd[244]: time=\"{}\" level=info msg=\"ignoring event\" container={id} module=libcontainerd namespace=moby topic=/tasks/delete type=\"*events.TaskDelete\"", docker_time(rng, time))),
                    plain(format!("containerd[211]: time=\"{}\" level=info msg=\"shim disconnected\" id={id} namespace=moby", docker_time(rng, time))),
                    plain(format!("containerd[211]: time=\"{}\" level=info msg=\"cleaning up after shim disconnected\" id={id} namespace=moby", docker_time(rng, time))),
                ]
            }
            30..=39 => vec![
                plain("systemd[1]: Starting sysstat-collect.service - system activity accounting tool...".to_owned()),
                plain("systemd[1]: sysstat-collect.service: Deactivated successfully.".to_owned()),
                plain("systemd[1]: Finished sysstat-collect.service - system activity accounting tool.".to_owned()),
            ],
            40..=47 => {
                let pid = self.pid(rng);
                let command = rng.pick(&["/usr/bin/apt update", "/usr/bin/apt install -y redis-server", "/usr/bin/systemctl restart nginx", "/usr/bin/tail -n 50 /var/log/nginx/error.log"]);
                vec![
                    plain(format!("sudo[{pid}]:    {user} : TTY=pts/0 ; PWD=/home/{user} ; USER=root ; COMMAND={command}")),
                    plain(format!("sudo[{pid}]: pam_unix(sudo:session): session opened for user root(uid=0) by {user}(uid=1000)")),
                    plain(format!("sudo[{pid}]: pam_unix(sudo:session): session closed for user root")),
                ]
            }
            48..=55 => vec![
                plain(format!("systemd[1]: Reloading requested from client PID {} ('systemctl') (unit session-{}.scope)...", self.pid(rng), self.session)),
                plain("systemd[1]: Reloading...".to_owned()),
                plain(format!("systemd[1]: Reloading finished in {} ms.", rng.range(180, 420))),
            ],
            56..=62 => vec![plain("kernel: mini_init (2): drop_caches: 1".to_owned())],
            63..=68 => vec![
                plain("systemd[1]: Starting apt-daily.service - Daily apt download activities...".to_owned()),
                plain("systemd[1]: apt-daily.service: Deactivated successfully.".to_owned()),
                plain("systemd[1]: Finished apt-daily.service - Daily apt download activities.".to_owned()),
            ],
            69..=74 => {
                self.session += 1;
                vec![
                    plain(format!("systemd-logind[178]: New session {} of user {user}.", self.session)),
                    plain(format!("systemd[1]: Started session-{0}.scope - Session {0} of User {user}.", self.session)),
                ]
            }
            75..=79 => vec![plain(format!(
                "snapd[191]: storehelpers.go:{}: cannot refresh: snap has no updates available: \"core22\", \"lxd\", \"snapd\"",
                rng.range(900, 960)
            ))],
            80..=84 => vec![plain("systemd-resolved[143]: Clock change detected. Flushing caches.".to_owned())],
            85..=89 => vec![
                plain("systemd[1]: Starting logrotate.service - Rotate log files...".to_owned()),
                plain("systemd[1]: logrotate.service: Deactivated successfully.".to_owned()),
                plain("systemd[1]: Finished logrotate.service - Rotate log files.".to_owned()),
            ],
            90..=94 => vec![(
                "kernel: hv_balloon: Balloon request will be partially fulfilled. Balloon floor reached.".to_owned(),
                Style::fg(WARNING),
            )],
            95..=97 => vec![(
                format!("dockerd[244]: time=\"{}\" level=warning msg=\"Health check for container {} error: context deadline exceeded\"", docker_time(rng, time), container(rng)),
                Style::fg(WARNING),
            )],
            _ => vec![(
                format!("kernel: hv_storvsc fd1d2cbd-ce7c-535c-966b-eb5f811c95f0: tag#{} cmd 0x2a status: scsi 0x2 srb 0x4 hv 0xc0000001", rng.range(0, 64)),
                Style::fg(BRIGHT_RED),
            )],
        }
    }

    fn line(&self, time: DateTime<Local>, host: &str, (text, style): (String, Style)) -> Line {
        Line::new().with(format!("{} {host} ", time.format("%b %d %H:%M:%S")), Style::PLAIN).with(text, style)
    }
}

impl super::Script for Journal {
    fn next(&mut self, cx: &mut Cx) -> Plan {
        let mut plan = Plan::default();
        let host = cx.machine.host.clone();
        let user = cx.machine.user.clone();
        if !std::mem::replace(&mut self.began, true) {
            // journalctl starts with the last ten lines from before.
            plan.typed(cx.machine, "journalctl -f");
            // Events from the last half hour or so, going back in time.
            let mut events = Vec::new();
            let mut time = cx.now - Duration::seconds(cx.rng.range(20, 90) as i64);
            while events.iter().map(|(_, lines): &(_, Vec<_>)| lines.len()).sum::<usize>() < 10 {
                events.push((time, self.event(cx.rng, &user, time)));
                time -= Duration::seconds(cx.rng.range(30, 400) as i64);
            }
            let lines: Vec<Line> = events
                .into_iter()
                .rev()
                .flat_map(|(time, entries)| entries.into_iter().map(move |entry| (time, entry)))
                .map(|(time, entry)| self.line(time, &host, entry))
                .collect();
            // The last ten of them, oldest at the top.
            for line in lines.into_iter().rev().take(10).collect::<Vec<_>>().into_iter().rev() {
                plan.print(line);
            }
            return plan;
        }
        // Quiet for a while, then a burst of a few lines.
        let quiet = cx.rng.range(1_500, 14_000);
        plan.wait(quiet);
        let time = cx.now + Duration::milliseconds(quiet as i64);
        for entry in self.event(cx.rng, &user, time) {
            plan.wait(cx.rng.range(0, 40)).print(self.line(time, &host, entry));
        }
        plan
    }
}

/// A Docker container id, which the log gives in full.
fn container(rng: &mut Rng) -> String {
    (0..4).map(|_| format!("{:016x}", rng.next())).collect()
}

/// A Docker log time a moment before `time`, in Go's RFC 3339 with
/// nanoseconds, which leaves off trailing zeros.
fn docker_time(rng: &mut Rng, time: DateTime<Local>) -> String {
    let time = time - Duration::milliseconds(rng.range(0, 900) as i64);
    let nanos = format!("{:09}", rng.range(1, 1_000_000_000));
    format!("{}.{}{}", time.format("%Y-%m-%dT%H:%M:%S"), nanos.trim_end_matches('0'), time.format("%:z"))
}

#[cfg(test)]
mod tests {
    use super::super::{Feed, Machine};
    use super::*;

    #[test]
    fn it_opens_with_ten_lines_and_keeps_going() {
        let machine = Machine { user: "peter".to_owned(), host: "DESKTOP-4KQ2".to_owned(), booted: Local::now() };
        let mut feed = Feed::new(Box::new(Journal::new()), machine, 9);
        let now = Local::now();
        feed.run_to(std::time::Duration::ZERO, now);
        let text = feed.all_text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "peter@DESKTOP-4KQ2:~$ journalctl -f");
        assert_eq!(lines.len(), 11);
        assert!(lines[1..].iter().all(|line| line.contains(" DESKTOP-4KQ2 ")));
        // Ten minutes later, caught up all at once.
        let later = now + chrono::Duration::minutes(10);
        feed.run_to(std::time::Duration::from_secs(600), later);
        let text = feed.all_text();
        assert!(text.lines().count() > 60, "a few lines a minute");
        let stamps: Vec<&str> = text.lines().skip(1).map(|line| &line[..15]).collect();
        assert!(stamps.windows(2).all(|pair| pair[0] <= pair[1]), "time only goes forward: {stamps:?}");
        let (first, last) = (now.format("%b %d %H:%M:%S").to_string(), later.format("%b %d %H:%M:%S").to_string());
        assert!(stamps[9] <= first.as_str() && stamps[10] >= first.as_str(), "ten lines from before, then new ones");
        assert!(stamps.last().unwrap() <= &last.as_str(), "nothing from the future");
    }
}
