//! The main screen: apt keeping an Ubuntu 24.04 machine up to date and
//! setting up a few things on it, with the odd look around in between. The
//! first time through it updates, upgrades and installs; after that it purges
//! what it installed and installs it again, as when testing a setup script.

use std::time::Duration;

use chrono::{DateTime, Local};

use super::{Cx, GREEN, Line, Plan, Status, Style, size};

const ARCHIVE: &str = "http://archive.ubuntu.com/ubuntu";
const SECURITY: &str = "http://security.ubuntu.com/ubuntu";
/// How long sudo remembers the password.
const SUDO_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// Time at the prompt before the next command, in milliseconds.
const IDLE: (u64, u64) = (2_500, 14_000);

/// A package as apt shows it.
struct Package {
    name: &'static str,
    version: &'static str,
    /// The version it replaces, for upgrades.
    old: &'static str,
    arch: &'static str,
    /// Where it comes from, e.g. `noble-updates/main`.
    pocket: &'static str,
    /// The download, in bytes.
    size: u64,
}

const fn new(name: &'static str, version: &'static str, arch: &'static str, pocket: &'static str, size: u64) -> Package {
    Package { name, version, old: "", arch, pocket, size }
}

const fn up(name: &'static str, old: &'static str, version: &'static str, arch: &'static str, pocket: &'static str, size: u64) -> Package {
    Package { name, version, old, arch, pocket, size }
}

/// Something installed with one command, and what comes with it.
struct Bundle {
    /// What's typed after `apt install -y`.
    target: &'static str,
    /// Everything installed, in dpkg's order; the target is among them.
    packages: &'static [Package],
    suggested: &'static str,
    /// What packages print as they're set up.
    setup: &'static [(&'static str, &'static [&'static str])],
    /// The triggers that run at the end.
    triggers: &'static [&'static str],
}

const MAN_DB: &str = "man-db (2.12.0-4build2)";
const LIBC_BIN: &str = "libc-bin (2.39-0ubuntu8.5)";

const UPGRADES: &[Package] = &[
    up("base-files", "13ubuntu10.2", "13ubuntu10.3", "amd64", "noble-updates/main", 73_500),
    up("libc6", "2.39-0ubuntu8.4", "2.39-0ubuntu8.5", "amd64", "noble-updates/main", 3_262_000),
    up("libc-bin", "2.39-0ubuntu8.4", "2.39-0ubuntu8.5", "amd64", "noble-updates/main", 682_000),
    up("locales", "2.39-0ubuntu8.4", "2.39-0ubuntu8.5", "all", "noble-updates/main", 4_230_000),
    up("libssl3t64", "3.0.13-0ubuntu3.5", "3.0.13-0ubuntu3.6", "amd64", "noble-security/main", 1_940_000),
    up("openssl", "3.0.13-0ubuntu3.5", "3.0.13-0ubuntu3.6", "amd64", "noble-security/main", 1_002_000),
    up("openssh-client", "1:9.6p1-3ubuntu13.11", "1:9.6p1-3ubuntu13.12", "amd64", "noble-security/main", 906_000),
    up("openssh-server", "1:9.6p1-3ubuntu13.11", "1:9.6p1-3ubuntu13.12", "amd64", "noble-security/main", 510_000),
    up("openssh-sftp-server", "1:9.6p1-3ubuntu13.11", "1:9.6p1-3ubuntu13.12", "amd64", "noble-security/main", 37_300),
    up("libsystemd0", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 434_000),
    up("libsystemd-shared", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 2_070_000),
    up("libudev1", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 176_000),
    up("systemd", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 3_475_000),
    up("systemd-sysv", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 11_900),
    up("systemd-dev", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "all", "noble-updates/main", 105_000),
    up("systemd-resolved", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 296_000),
    up("systemd-timesyncd", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 35_300),
    up("udev", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 1_873_000),
    up("libpam-systemd", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 235_000),
    up("libnss-systemd", "255.4-1ubuntu8.8", "255.4-1ubuntu8.10", "amd64", "noble-updates/main", 159_000),
    up("python3.12", "3.12.3-1ubuntu0.7", "3.12.3-1ubuntu0.8", "amd64", "noble-security/main", 651_000),
    up("libpython3.12t64", "3.12.3-1ubuntu0.7", "3.12.3-1ubuntu0.8", "amd64", "noble-security/main", 2_339_000),
    up("libpython3.12-minimal", "3.12.3-1ubuntu0.7", "3.12.3-1ubuntu0.8", "amd64", "noble-security/main", 832_000),
    up("libpython3.12-stdlib", "3.12.3-1ubuntu0.7", "3.12.3-1ubuntu0.8", "amd64", "noble-security/main", 2_068_000),
    up("python3.12-minimal", "3.12.3-1ubuntu0.7", "3.12.3-1ubuntu0.8", "amd64", "noble-security/main", 2_331_000),
    up("curl", "8.5.0-2ubuntu10.6", "8.5.0-2ubuntu10.7", "amd64", "noble-security/main", 227_000),
    up("libcurl4t64", "8.5.0-2ubuntu10.6", "8.5.0-2ubuntu10.7", "amd64", "noble-security/main", 341_000),
    up("libcurl3t64-gnutls", "8.5.0-2ubuntu10.6", "8.5.0-2ubuntu10.7", "amd64", "noble-security/main", 333_000),
    up("tzdata", "2025b-0ubuntu0.24.04.1", "2025c-0ubuntu0.24.04", "all", "noble-updates/main", 276_000),
    up("git", "1:2.43.0-1ubuntu7.2", "1:2.43.0-1ubuntu7.3", "amd64", "noble-security/main", 3_679_000),
    up("git-man", "1:2.43.0-1ubuntu7.2", "1:2.43.0-1ubuntu7.3", "all", "noble-security/main", 1_100_000),
    up("vim", "2:9.1.0016-1ubuntu7.8", "2:9.1.0016-1ubuntu7.9", "amd64", "noble-updates/main", 1_881_000),
    up("vim-common", "2:9.1.0016-1ubuntu7.8", "2:9.1.0016-1ubuntu7.9", "all", "noble-updates/main", 385_000),
    up("vim-runtime", "2:9.1.0016-1ubuntu7.8", "2:9.1.0016-1ubuntu7.9", "all", "noble-updates/main", 7_288_000),
    up("vim-tiny", "2:9.1.0016-1ubuntu7.8", "2:9.1.0016-1ubuntu7.9", "amd64", "noble-updates/main", 804_000),
    up("xxd", "2:9.1.0016-1ubuntu7.8", "2:9.1.0016-1ubuntu7.9", "amd64", "noble-updates/main", 70_400),
    up("distro-info-data", "0.60ubuntu0.3", "0.60ubuntu0.4", "all", "noble-updates/main", 6_810),
    up("ubuntu-pro-client", "35.1ubuntu0~24.04", "36ubuntu0~24.04", "amd64", "noble-updates/main", 239_000),
    up("ubuntu-pro-client-l10n", "35.1ubuntu0~24.04", "36ubuntu0~24.04", "amd64", "noble-updates/main", 19_000),
    up("snapd", "2.67.1+24.04", "2.68.5+ubuntu24.04.1", "amd64", "noble-updates/main", 30_600_000),
];

const BUILD: Bundle = Bundle {
    target: "build-essential",
    packages: &[
        new("binutils-common", "2.42-4ubuntu2.5", "amd64", "noble-updates/main", 240_000),
        new("libsframe1", "2.42-4ubuntu2.5", "amd64", "noble-updates/main", 14_800),
        new("libbinutils", "2.42-4ubuntu2.5", "amd64", "noble-updates/main", 576_000),
        new("libctf-nobfd0", "2.42-4ubuntu2.5", "amd64", "noble-updates/main", 97_600),
        new("libctf0", "2.42-4ubuntu2.5", "amd64", "noble-updates/main", 94_500),
        new("libgprofng0", "2.42-4ubuntu2.5", "amd64", "noble-updates/main", 849_000),
        new("libjansson4", "2.14-2build2", "amd64", "noble/main", 32_700),
        new("binutils-x86-64-linux-gnu", "2.42-4ubuntu2.5", "amd64", "noble-updates/main", 2_463_000),
        new("binutils", "2.42-4ubuntu2.5", "amd64", "noble-updates/main", 18_100),
        new("libc-dev-bin", "2.39-0ubuntu8.5", "amd64", "noble-updates/main", 20_400),
        new("linux-libc-dev", "6.8.0-87.88", "amd64", "noble-updates/main", 1_934_000),
        new("libcrypt-dev", "1:4.4.36-4build1", "amd64", "noble/main", 112_000),
        new("rpcsvc-proto", "1.4.2-0ubuntu7", "amd64", "noble/main", 67_200),
        new("libc6-dev", "2.39-0ubuntu8.5", "amd64", "noble-updates/main", 2_125_000),
        new("gcc-13-base", "13.3.0-6ubuntu2~24.04", "amd64", "noble-updates/main", 51_600),
        new("libisl23", "0.26-3build1.1", "amd64", "noble/main", 680_000),
        new("libmpc3", "1.3.1-1build1.1", "amd64", "noble/main", 54_600),
        new("cpp-13-x86-64-linux-gnu", "13.3.0-6ubuntu2~24.04", "amd64", "noble-updates/main", 10_700_000),
        new("cpp-13", "13.3.0-6ubuntu2~24.04", "amd64", "noble-updates/main", 1_038),
        new("cpp-x86-64-linux-gnu", "4:13.2.0-7ubuntu1", "amd64", "noble/main", 5_326),
        new("cpp", "4:13.2.0-7ubuntu1", "amd64", "noble/main", 22_400),
        new("libcc1-0", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 47_700),
        new("libgomp1", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 148_000),
        new("libitm1", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 29_700),
        new("libatomic1", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 10_500),
        new("libasan8", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 2_998_000),
        new("liblsan0", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 1_317_000),
        new("libtsan2", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 2_733_000),
        new("libubsan1", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 1_175_000),
        new("libhwasan0", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 1_632_000),
        new("libquadmath0", "14.2.0-4ubuntu2~24.04", "amd64", "noble-updates/main", 153_000),
        new("libgcc-13-dev", "13.3.0-6ubuntu2~24.04", "amd64", "noble-updates/main", 2_681_000),
        new("gcc-13-x86-64-linux-gnu", "13.3.0-6ubuntu2~24.04", "amd64", "noble-updates/main", 21_100_000),
        new("gcc-13", "13.3.0-6ubuntu2~24.04", "amd64", "noble-updates/main", 494_000),
        new("gcc-x86-64-linux-gnu", "4:13.2.0-7ubuntu1", "amd64", "noble/main", 1_212),
        new("gcc", "4:13.2.0-7ubuntu1", "amd64", "noble/main", 5_018),
        new("libstdc++-13-dev", "13.3.0-6ubuntu2~24.04", "amd64", "noble-updates/main", 2_420_000),
        new("g++-13-x86-64-linux-gnu", "13.3.0-6ubuntu2~24.04", "amd64", "noble-updates/main", 12_200_000),
        new("g++-13", "13.3.0-6ubuntu2~24.04", "amd64", "noble-updates/main", 16_100),
        new("g++-x86-64-linux-gnu", "4:13.2.0-7ubuntu1", "amd64", "noble/main", 964),
        new("g++", "4:13.2.0-7ubuntu1", "amd64", "noble/main", 1_100),
        new("make", "4.3-4.1build2", "amd64", "noble/main", 180_000),
        new("libdpkg-perl", "1.22.6ubuntu6.5", "all", "noble-updates/main", 269_000),
        new("patch", "2.7.6-7build3", "amd64", "noble/main", 104_000),
        new("lto-disabled-list", "47", "all", "noble/main", 12_400),
        new("dpkg-dev", "1.22.6ubuntu6.5", "all", "noble-updates/main", 1_074_000),
        new("build-essential", "12.10ubuntu1", "amd64", "noble/main", 4_928),
        new("libfakeroot", "1.33-1", "amd64", "noble/main", 32_200),
        new("fakeroot", "1.33-1", "amd64", "noble/main", 67_200),
        new("libalgorithm-diff-perl", "1.201-1", "all", "noble/main", 41_800),
        new("libalgorithm-diff-xs-perl", "0.04-8build3", "amd64", "noble/main", 11_200),
        new("libalgorithm-merge-perl", "0.08-5", "all", "noble/main", 11_400),
        new("libfile-fcntllock-perl", "0.22-4ubuntu5", "amd64", "noble/main", 30_700),
        new("manpages-dev", "6.7-2", "all", "noble/main", 2_013_000),
    ],
    suggested: "binutils-doc gprofng-gui cpp-doc gcc-13-locales cpp-13-doc debian-keyring g++-multilib g++-13-multilib gcc-13-doc gcc-multilib autoconf automake libtool flex bison gdb gcc-doc gcc-13-multilib gdb-x86-64-linux-gnu glibc-doc bzr libstdc++-13-doc make-doc ed diffutils-doc",
    setup: &[
        ("gcc", &["update-alternatives: using /usr/bin/gcc to provide /usr/bin/cc (cc) in auto mode"]),
        ("g++", &["update-alternatives: using /usr/bin/g++ to provide /usr/bin/c++ (c++) in auto mode"]),
        ("fakeroot", &["update-alternatives: using /usr/bin/fakeroot-sysv to provide /usr/bin/fakeroot (fakeroot) in auto mode"]),
    ],
    triggers: &[MAN_DB, LIBC_BIN],
};

const REDIS: Bundle = Bundle {
    target: "redis-server",
    packages: &[
        new("libjemalloc2", "5.3.0-2build1", "amd64", "noble/universe", 256_000),
        new("liblzf1", "3.6-4", "amd64", "noble/universe", 7_624),
        new("redis-tools", "5:7.0.15-1ubuntu0.24.04.2", "amd64", "noble-updates/universe", 1_166_000),
        new("redis-server", "5:7.0.15-1ubuntu0.24.04.2", "amd64", "noble-updates/universe", 45_900),
    ],
    suggested: "ruby-redis",
    setup: &[(
        "redis-server",
        &[
            "Created symlink /etc/systemd/system/redis.service → /usr/lib/systemd/system/redis-server.service.",
            "Created symlink /etc/systemd/system/multi-user.target.wants/redis-server.service → /usr/lib/systemd/system/redis-server.service.",
        ],
    )],
    triggers: &[MAN_DB, LIBC_BIN],
};

const FAIL2BAN: Bundle = Bundle {
    target: "fail2ban",
    packages: &[
        new("python3-pyasyncore", "1.0.2-2", "all", "noble/main", 10_100),
        new("python3-pyinotify", "0.9.6-2ubuntu1", "all", "noble/main", 25_000),
        new("whois", "5.5.22", "amd64", "noble/main", 51_700),
        new("python3-systemd", "235-1build4", "amd64", "noble/main", 42_400),
        new("fail2ban", "1.0.2-3ubuntu0.1", "all", "noble-updates/universe", 409_000),
    ],
    suggested: "mailx monit sqlite3 python-pyinotify-doc",
    setup: &[(
        "fail2ban",
        &[
            "/usr/lib/python3/dist-packages/fail2ban/tests/fail2banregextestcase.py:224: SyntaxWarning: invalid escape sequence '\\s'",
            "  \"1490349000 test failed.dns.ch\", \"^\\s*test <F-ID>\\S+</F-ID>\"",
            "/usr/lib/python3/dist-packages/fail2ban/tests/servertestcase.py:1375: SyntaxWarning: invalid escape sequence '\\s'",
            "  \"`{ nft -a list chain inet f2b-table f2b-chain | grep -oP '@addr-set-j-w-nft-mp\\s+.*\\s+\\Khandle\\s+(\\d+)$'; } | while read -r hdl; do`\",",
            "Created symlink /etc/systemd/system/multi-user.target.wants/fail2ban.service → /usr/lib/systemd/system/fail2ban.service.",
        ],
    )],
    triggers: &[MAN_DB],
};

const EXPORTER: Bundle = Bundle {
    target: "prometheus-node-exporter",
    packages: &[
        new("libio-pty-perl", "1:1.20-1build2", "amd64", "noble/main", 31_200),
        new("libipc-run-perl", "20231003.0-1", "all", "noble/main", 92_100),
        new("libtime-duration-perl", "1.21-2", "all", "noble/main", 12_300),
        new("libtimedate-perl", "2.3300-2", "all", "noble/main", 34_600),
        new("moreutils", "0.69-1", "amd64", "noble/universe", 56_100),
        new("prometheus-node-exporter-collectors", "0+git20230210.6f710f8-1", "all", "noble/universe", 17_800),
        new("smartmontools", "7.4-2build1", "amd64", "noble/main", 643_000),
        new("prometheus-node-exporter", "1.7.0-1ubuntu0.3", "amd64", "noble-updates/universe", 3_402_000),
    ],
    suggested: "gsmartcontrol smart-notifier mailx | mailutils",
    setup: &[
        (
            "smartmontools",
            &[
                "Created symlink /etc/systemd/system/smartd.service → /usr/lib/systemd/system/smartmontools.service.",
                "Created symlink /etc/systemd/system/multi-user.target.wants/smartmontools.service → /usr/lib/systemd/system/smartmontools.service.",
            ],
        ),
        (
            "prometheus-node-exporter-collectors",
            &[
                "Created symlink /etc/systemd/system/timers.target.wants/prometheus-node-exporter-apt.timer → /usr/lib/systemd/system/prometheus-node-exporter-apt.timer.",
                "Created symlink /etc/systemd/system/timers.target.wants/prometheus-node-exporter-ipmitool-sensor.timer → /usr/lib/systemd/system/prometheus-node-exporter-ipmitool-sensor.timer.",
                "Created symlink /etc/systemd/system/timers.target.wants/prometheus-node-exporter-mellanox-hca-temp.timer → /usr/lib/systemd/system/prometheus-node-exporter-mellanox-hca-temp.timer.",
                "Created symlink /etc/systemd/system/timers.target.wants/prometheus-node-exporter-nvme.timer → /usr/lib/systemd/system/prometheus-node-exporter-nvme.timer.",
                "Created symlink /etc/systemd/system/timers.target.wants/prometheus-node-exporter-smartmon.timer → /usr/lib/systemd/system/prometheus-node-exporter-smartmon.timer.",
            ],
        ),
        (
            "prometheus-node-exporter",
            &[
                "info: Selecting UID from range 100 to 999 ...",
                "",
                "info: Adding system user `prometheus' (UID 110) ...",
                "info: Adding new group `prometheus' (GID 112) ...",
                "info: Adding new user `prometheus' (UID 110) with group `prometheus' ...",
                "info: Not creating home directory `/var/lib/prometheus'.",
                "Created symlink /etc/systemd/system/multi-user.target.wants/prometheus-node-exporter.service → /usr/lib/systemd/system/prometheus-node-exporter.service.",
            ],
        ),
    ],
    triggers: &[MAN_DB],
};

const BUNDLES: [&Bundle; 4] = [&BUILD, &REDIS, &FAIL2BAN, &EXPORTER];

#[derive(Clone, Copy)]
enum Act {
    Update,
    Upgrade,
    Install(&'static Bundle),
    GccVersion,
    RedisStatus,
    DiskFree,
    Memory,
    Purge,
    Autoremove,
}

const FIRST_PASS: &[Act] = &[
    Act::Update,
    Act::Upgrade,
    Act::Install(&BUILD),
    Act::GccVersion,
    Act::Install(&REDIS),
    Act::RedisStatus,
    Act::Install(&FAIL2BAN),
    Act::Install(&EXPORTER),
    Act::DiskFree,
    Act::Autoremove,
];

const LATER_PASSES: &[Act] = &[
    Act::Update,
    Act::Purge,
    Act::Autoremove,
    Act::Install(&BUILD),
    Act::GccVersion,
    Act::Install(&REDIS),
    Act::RedisStatus,
    Act::Install(&FAIL2BAN),
    Act::Install(&EXPORTER),
    Act::Memory,
];

pub struct Apt {
    /// Whether the history on screen at the start has been written.
    began: bool,
    pass: usize,
    act: usize,
    /// When sudo will ask for the password again, on the feed's clock.
    sudo_until: Option<Duration>,
    /// What `apt update` found to upgrade.
    upgradable: Vec<&'static Package>,
    /// Packages installed by apt and not yet removed.
    installed: Vec<&'static Package>,
    redis_started: Option<(DateTime<Local>, u64)>,
    /// Files dpkg knows about, for its "Reading database" line.
    files: u64,
    next_pid: u64,
}

impl Apt {
    pub fn new() -> Self {
        Self {
            began: false,
            pass: 0,
            act: 0,
            sudo_until: None,
            upgradable: Vec::new(),
            installed: Vec::new(),
            redis_started: None,
            files: 41_893,
            next_pid: 4_812,
        }
    }

    fn pid(&mut self, cx: &mut Cx) -> u64 {
        self.next_pid += cx.rng.range(3, 60);
        self.next_pid
    }

    /// The commands from before, already on screen.
    fn history(&mut self, cx: &mut Cx) -> Plan {
        let mut plan = Plan::default();
        let up = cx.now - cx.machine.booted;
        plan.typed(cx.machine, "uptime");
        plan.print(format!(
            " {} up {} days, {:>2}:{:02},  1 user,  load average: 0.{:02}, 0.{:02}, 0.{:02}",
            cx.now.format("%H:%M:%S"),
            up.num_days(),
            up.num_hours() % 24,
            up.num_minutes() % 60,
            cx.rng.range(30, 70),
            cx.rng.range(30, 60),
            cx.rng.range(25, 50),
        ));
        plan.typed(cx.machine, "df -h");
        for line in DF {
            plan.print(*line);
        }
        plan.typed(cx.machine, "free -h");
        for line in FREE {
            plan.print(*line);
        }
        plan.typed(cx.machine, "systemctl is-active nginx postgresql docker");
        for _ in 0..3 {
            plan.print("active");
        }
        plan
    }

    /// `[sudo] password for peter:`, unless it was given lately.
    fn sudo(&mut self, plan: &mut Plan, cx: &mut Cx) {
        if self.sudo_until.is_some_and(|until| cx.clock < until) {
            return;
        }
        plan.wait(cx.rng.range(80, 200)).text(format!("[sudo] password for {}: ", cx.machine.user), Style::PLAIN);
        plan.wait(cx.rng.range(1_400, 2_800)).newline();
        self.sudo_until = Some(cx.clock + SUDO_TIMEOUT);
    }

    fn update(&mut self, cx: &mut Cx) -> Plan {
        let mut plan = Plan::default();
        plan.command(cx.rng, cx.machine, IDLE, "sudo apt update");
        self.sudo(&mut plan, cx);
        plan.status(Some(Status::Text("0% [Working]".to_owned())));
        let releases = [(ARCHIVE, "noble"), (ARCHIVE, "noble-updates"), (SECURITY, "noble-security"), (ARCHIVE, "noble-backports")];
        let mut n = 0;
        let mut fetched = 0;
        // The first time, everything has changed since the last update; after
        // that, nothing has.
        for (i, (url, release)) in releases.iter().enumerate() {
            n += 1;
            plan.wait(cx.rng.range(60, 400));
            if i == 0 || self.pass > 0 {
                plan.print(format!("Hit:{n} {url} {release} InRelease"));
            } else {
                fetched += 126_000;
                plan.print(format!("Get:{n} {url} {release} InRelease [126 kB]"));
            }
        }
        if self.pass == 0 {
            for (url, index, bytes) in INDEXES {
                n += 1;
                fetched += bytes;
                plan.status(Some(Status::Text(format!("{}% [{n} {index} {}]", n * 6, size(*bytes)))));
                plan.wait(cx.rng.range(80, 350) + bytes / 4_000).print(format!("Get:{n} {url} {index} [{}]", size(*bytes)));
            }
            let seconds = cx.rng.range(2, 5);
            plan.status(None).wait(cx.rng.range(100, 300));
            plan.print(format!("Fetched {} in {seconds}s ({}/s)", size(fetched), size(fetched / seconds)));
        } else {
            plan.status(None);
        }
        reading(&mut plan, cx);
        if self.pass == 0 {
            let count = cx.rng.range(24, UPGRADES.len() as u64 + 1) as usize;
            self.upgradable = UPGRADES.iter().take(count).collect();
            plan.print(format!("{count} packages can be upgraded. Run 'apt list --upgradable' to see them."));
        } else {
            plan.print("All packages are up to date.");
        }
        plan
    }

    fn upgrade(&mut self, cx: &mut Cx) -> Plan {
        let mut plan = Plan::default();
        plan.command(cx.rng, cx.machine, IDLE, "sudo apt upgrade -y");
        self.sudo(&mut plan, cx);
        reading(&mut plan, cx);
        plan.wait(cx.rng.range(200, 600)).print("Calculating upgrade... Done");
        let packages = std::mem::take(&mut self.upgradable);
        let mut names: Vec<&str> = packages.iter().map(|p| p.name).collect();
        names.sort_unstable();
        list(&mut plan, cx, "The following packages will be upgraded:", &names);
        let bytes: u64 = packages.iter().map(|p| p.size).sum();
        plan.print(format!("{} upgraded, 0 newly installed, 0 to remove and 0 not upgraded.", packages.len()));
        plan.print(format!("Need to get {} of archives.", size(bytes)));
        plan.print(format!("After this operation, {} of additional disk space will be used.", size(bytes / 9)));
        download(&mut plan, cx, &packages);
        plan.wait(cx.rng.range(200, 500)).print("Preconfiguring packages ...");
        let mut dpkg = Dpkg::new(packages.len());
        plan.status(Some(Status::Progress(0)));
        plan.wait(cx.rng.range(300, 700)).print(self.database());
        for (i, package) in packages.iter().enumerate() {
            plan.print(format!("Preparing to unpack .../{}", deb(i, package)));
            plan.wait(unpack_ms(cx, package));
            plan.print(format!("Unpacking {} ({}) over ({}) ...", package.name, package.version, package.old));
            dpkg.step(&mut plan);
        }
        for package in &packages {
            plan.wait(cx.rng.range(30, 220)).print(format!("Setting up {} ({}) ...", package.name, package.version));
            dpkg.step(&mut plan);
        }
        triggers(&mut plan, cx, &[MAN_DB, LIBC_BIN, "dbus (1.14.10-4ubuntu4.1)"]);
        plan.status(None);
        needrestart(&mut plan, cx);
        plan
    }

    fn install(&mut self, cx: &mut Cx, bundle: &'static Bundle) -> Plan {
        let mut plan = Plan::default();
        plan.command(cx.rng, cx.machine, IDLE, &format!("sudo apt install -y {}", bundle.target));
        self.sudo(&mut plan, cx);
        reading(&mut plan, cx);
        let packages: Vec<&Package> = bundle.packages.iter().collect();
        let additional: Vec<&str> = packages.iter().map(|p| p.name).filter(|name| *name != bundle.target).collect();
        let mut names: Vec<&str> = packages.iter().map(|p| p.name).collect();
        names.sort_unstable();
        if !additional.is_empty() {
            let mut sorted = additional.clone();
            sorted.sort_unstable();
            list(&mut plan, cx, "The following additional packages will be installed:", &sorted);
        }
        let suggested: Vec<&str> = bundle.suggested.split(' ').collect();
        list(&mut plan, cx, "Suggested packages:", &suggested);
        list(&mut plan, cx, "The following NEW packages will be installed:", &names);
        let bytes: u64 = packages.iter().map(|p| p.size).sum();
        plan.print(format!("0 upgraded, {} newly installed, 0 to remove and 0 not upgraded.", packages.len()));
        plan.print(format!("Need to get {} of archives.", size(bytes)));
        plan.print(format!("After this operation, {} of additional disk space will be used.", size(bytes * 4)));
        download(&mut plan, cx, &packages);
        let mut dpkg = Dpkg::new(packages.len());
        plan.status(Some(Status::Progress(0)));
        for (i, package) in packages.iter().enumerate() {
            plan.wait(cx.rng.range(20, 120)).print(format!("Selecting previously unselected package {}.", package.name));
            if i == 0 {
                plan.wait(cx.rng.range(300, 700)).print(self.database());
            }
            plan.print(format!("Preparing to unpack .../{}", deb(i, package)));
            plan.wait(unpack_ms(cx, package));
            plan.print(format!("Unpacking {} ({}) ...", package.name, package.version));
            dpkg.step(&mut plan);
        }
        for package in &packages {
            plan.wait(cx.rng.range(30, 260)).print(format!("Setting up {} ({}) ...", package.name, package.version));
            for (name, lines) in bundle.setup {
                if *name == package.name {
                    for line in *lines {
                        plan.wait(cx.rng.range(40, 400)).print(*line);
                    }
                }
            }
            dpkg.step(&mut plan);
        }
        if bundle.target == REDIS.target {
            let pid = self.pid(cx);
            self.redis_started = Some((cx.now, pid));
        }
        self.files += packages.len() as u64 * 97;
        self.installed.extend(packages);
        triggers(&mut plan, cx, bundle.triggers);
        plan.status(None);
        needrestart(&mut plan, cx);
        plan
    }

    fn purge(&mut self, cx: &mut Cx) -> Plan {
        let targets: Vec<&str> = BUNDLES.iter().map(|b| b.target).collect();
        let mut plan = Plan::default();
        plan.command(cx.rng, cx.machine, IDLE, &format!("sudo apt purge -y {}", targets.join(" ")));
        self.sudo(&mut plan, cx);
        reading(&mut plan, cx);
        let mut leftover: Vec<&str> =
            self.installed.iter().map(|p| p.name).filter(|name| !targets.contains(name)).collect();
        leftover.sort_unstable();
        list(&mut plan, cx, "The following packages were automatically installed and are no longer required:", &leftover);
        plan.print("Use 'sudo apt autoremove' to remove them.");
        let mut removed: Vec<&'static Package> = self.installed.iter().copied().filter(|p| targets.contains(&p.name)).collect();
        removed.sort_unstable_by_key(|p| p.name);
        let starred: Vec<String> = removed.iter().map(|p| format!("{}*", p.name)).collect();
        let starred: Vec<&str> = starred.iter().map(String::as_str).collect();
        list(&mut plan, cx, "The following packages will be REMOVED:", &starred);
        self.remove(&mut plan, cx, &removed);
        plan
    }

    fn autoremove(&mut self, cx: &mut Cx) -> Plan {
        let mut plan = Plan::default();
        let command = if self.pass == 0 { "sudo apt autoremove -y" } else { "sudo apt autoremove --purge -y" };
        plan.command(cx.rng, cx.machine, IDLE, command);
        self.sudo(&mut plan, cx);
        reading(&mut plan, cx);
        let mut removed: Vec<&'static Package> = if self.pass == 0 { Vec::new() } else { self.installed.clone() };
        if removed.is_empty() {
            plan.print("0 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.");
            return plan;
        }
        removed.sort_unstable_by_key(|p| p.name);
        let starred: Vec<String> = removed.iter().map(|p| format!("{}*", p.name)).collect();
        let starred: Vec<&str> = starred.iter().map(String::as_str).collect();
        list(&mut plan, cx, "The following packages will be REMOVED:", &starred);
        self.remove(&mut plan, cx, &removed);
        plan
    }

    /// Removing and purging, once apt has said what will go.
    fn remove(&mut self, plan: &mut Plan, cx: &mut Cx, removed: &[&'static Package]) {
        let bytes: u64 = removed.iter().map(|p| p.size * 4).sum();
        plan.print(format!("0 upgraded, 0 newly installed, {} to remove and 0 not upgraded.", removed.len()));
        plan.print(format!("After this operation, {} disk space will be freed.", size(bytes)));
        let mut dpkg = Dpkg::new(removed.len());
        plan.status(Some(Status::Progress(0)));
        plan.wait(cx.rng.range(300, 700)).print(self.database());
        for package in removed {
            plan.wait(cx.rng.range(40, 300)).print(format!("Removing {} ({}) ...", package.name, package.version));
            dpkg.step(plan);
        }
        triggers(plan, cx, &[MAN_DB]);
        self.files -= (removed.len() as u64 * 97).min(self.files);
        let configured = removed.iter().filter(|p| CONFFILES.contains(&p.name));
        let mut first = true;
        for package in configured {
            if std::mem::take(&mut first) {
                plan.wait(cx.rng.range(300, 700)).print(self.database());
            }
            plan.wait(cx.rng.range(60, 300)).print(format!("Purging configuration files for {} ({}) ...", package.name, package.version));
        }
        for _ in removed {
            dpkg.step(plan);
        }
        self.installed.retain(|p| !removed.iter().any(|r| r.name == p.name));
        if !self.installed.iter().any(|p| p.name == REDIS.target) {
            self.redis_started = None;
        }
        plan.status(None);
        needrestart(plan, cx);
    }

    fn gcc_version(&mut self, cx: &mut Cx) -> Plan {
        let mut plan = Plan::default();
        plan.command(cx.rng, cx.machine, IDLE, "gcc --version");
        plan.wait(cx.rng.range(20, 60));
        for line in [
            "gcc (Ubuntu 13.3.0-6ubuntu2~24.04) 13.3.0",
            "Copyright (C) 2023 Free Software Foundation, Inc.",
            "This is free software; see the source for copying conditions.  There is NO",
            "warranty; not even for MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.",
            "",
        ] {
            plan.print(line);
        }
        plan
    }

    /// `systemctl status redis-server`, as it looks a little while after
    /// it was installed.
    fn redis_status(&mut self, cx: &mut Cx) -> Plan {
        let mut plan = Plan::default();
        plan.command(cx.rng, cx.machine, IDLE, "systemctl status redis-server --no-pager");
        let (since, pid) = match self.redis_started {
            Some(started) => started,
            None => {
                let pid = self.pid(cx);
                (cx.now, pid)
            }
        };
        // It's typed after a pause, so it's been running a little longer.
        let since = since.min(cx.now - chrono::Duration::seconds(cx.rng.range(4, 9) as i64));
        let ago = ago(cx.now - since);
        let bullet = Style::fg(GREEN);
        plan.wait(cx.rng.range(60, 200));
        plan.print(Line::new().with("●", bullet).with(" redis-server.service - Advanced key-value store", Style::PLAIN));
        plan.print("     Loaded: loaded (/usr/lib/systemd/system/redis-server.service; enabled; preset: enabled)");
        plan.print(
            Line::plain("     Active: ")
                .with("active (running)", bullet)
                .with(format!(" since {}; {ago} ago", systemd_time(since)), Style::PLAIN),
        );
        plan.print("       Docs: http://redis.io/documentation,");
        plan.print("             man:redis-server(1)");
        plan.print(format!("   Main PID: {pid} (redis-server)"));
        plan.print("     Status: \"Ready to accept connections\"");
        plan.print(format!("      Tasks: 5 (limit: {})", 18_949));
        plan.print(format!("     Memory: {:.1}M (peak: {:.1}M)", 3.0 + cx.rng.unit(), 4.0 + cx.rng.unit()));
        plan.print(format!("        CPU: {}ms", cx.rng.range(30, 90)));
        plan.print("     CGroup: /system.slice/redis-server.service");
        plan.print(format!("             └─{pid} \"/usr/bin/redis-server 127.0.0.1:6379\""));
        plan.print("");
        let stamp = since.format("%b %d %H:%M:%S");
        let host = &cx.machine.host;
        plan.print(format!("{stamp} {host} systemd[1]: Starting redis-server.service - Advanced key-value store..."));
        plan.print(format!("{stamp} {host} systemd[1]: Started redis-server.service - Advanced key-value store."));
        plan
    }

    fn disk_free(&mut self, cx: &mut Cx) -> Plan {
        let mut plan = Plan::default();
        plan.command(cx.rng, cx.machine, IDLE, "df -h /");
        plan.wait(cx.rng.range(20, 60));
        plan.print(DF[0]);
        plan.print(DF[4].replace("9.6G", "9.9G").replace("947G", "946G"));
        plan
    }

    fn memory(&mut self, cx: &mut Cx) -> Plan {
        let mut plan = Plan::default();
        plan.command(cx.rng, cx.machine, IDLE, "free -h");
        plan.wait(cx.rng.range(20, 60));
        for line in FREE {
            plan.print(*line);
        }
        plan
    }

    fn database(&self) -> String {
        format!("(Reading database ... {} files and directories currently installed.)", self.files)
    }
}

impl super::Script for Apt {
    fn next(&mut self, cx: &mut Cx) -> Plan {
        if !std::mem::replace(&mut self.began, true) {
            return self.history(cx);
        }
        let acts = if self.pass == 0 { FIRST_PASS } else { LATER_PASSES };
        let act = acts[self.act];
        let plan = match act {
            Act::Update => self.update(cx),
            Act::Upgrade => self.upgrade(cx),
            Act::Install(bundle) => self.install(cx, bundle),
            Act::GccVersion => self.gcc_version(cx),
            Act::RedisStatus => self.redis_status(cx),
            Act::DiskFree => self.disk_free(cx),
            Act::Memory => self.memory(cx),
            Act::Purge => self.purge(cx),
            Act::Autoremove => self.autoremove(cx),
        };
        self.act += 1;
        if self.act == acts.len() {
            self.act = 0;
            self.pass += 1;
        }
        plan
    }
}

/// Packages that leave configuration behind until they're purged.
const CONFFILES: &[&str] = &["redis-server", "redis-tools", "fail2ban", "prometheus-node-exporter", "smartmontools"];

/// The package indexes `apt update` downloads when there's news.
const INDEXES: &[(&str, &str, u64)] = &[
    (ARCHIVE, "noble-updates/main amd64 Packages", 1_402_000),
    (ARCHIVE, "noble-updates/main Translation-en", 281_000),
    (ARCHIVE, "noble-updates/main amd64 Components", 175_000),
    (ARCHIVE, "noble-updates/universe amd64 Packages", 1_176_000),
    (ARCHIVE, "noble-updates/universe Translation-en", 298_000),
    (ARCHIVE, "noble-updates/universe amd64 Components", 377_000),
    (ARCHIVE, "noble-updates/restricted amd64 Packages", 1_812_000),
    (ARCHIVE, "noble-updates/restricted Translation-en", 397_000),
    (SECURITY, "noble-security/main amd64 Packages", 1_128_000),
    (SECURITY, "noble-security/main Translation-en", 199_000),
    (SECURITY, "noble-security/main amd64 Components", 21_600),
    (SECURITY, "noble-security/universe amd64 Packages", 887_000),
];

/// `df -h` on WSL.
const DF: &[&str] = &[
    "Filesystem      Size  Used Avail Use% Mounted on",
    "none            7.8G     0  7.8G   0% /usr/lib/modules/6.6.87.2-microsoft-standard-WSL2",
    "none            7.8G  4.0K  7.8G   1% /mnt/wsl",
    "drivers         476G  318G  158G  67% /usr/lib/wsl/drivers",
    "/dev/sdd       1007G  9.6G  947G   2% /",
    "none            7.8G   84K  7.8G   1% /mnt/wslg",
    "none            7.8G     0  7.8G   0% /usr/lib/wsl/lib",
    "rootfs          7.8G  2.7M  7.8G   1% /init",
    "none            7.8G  516K  7.8G   1% /run",
    "none            7.8G     0  7.8G   0% /run/lock",
    "none            7.8G     0  7.8G   0% /run/shm",
    "none            7.8G   76K  7.8G   1% /mnt/wslg/versions.txt",
    "none            7.8G   76K  7.8G   1% /mnt/wslg/doc",
    "C:\\             476G  318G  158G  67% /mnt/c",
    "tmpfs           1.6G   16K  1.6G   1% /run/user/1000",
];

const FREE: &[&str] = &[
    "               total        used        free      shared  buff/cache   available",
    "Mem:            15Gi       1.9Gi        13Gi       3.6Mi       1.1Gi        13Gi",
    "Swap:          4.0Gi          0B       4.0Gi",
];

/// `Reading package lists... Done` and the two after it.
fn reading(plan: &mut Plan, cx: &mut Cx) {
    for what in ["Reading package lists...", "Building dependency tree...", "Reading state information..."] {
        plan.wait(cx.rng.range(80, 250)).text(what, Style::PLAIN);
        plan.wait(cx.rng.range(150, 900)).text(" Done", Style::PLAIN).newline();
    }
}

/// A heading and package names under it, laid out to the terminal's width
/// as apt does.
fn list(plan: &mut Plan, cx: &Cx, heading: &str, names: &[&str]) {
    if names.is_empty() {
        return;
    }
    plan.print(heading);
    let width = cx.cols.saturating_sub(1).max(40);
    let mut line = " ".to_owned();
    for name in names {
        if line.len() > 1 && line.len() + 1 + name.len() > width {
            plan.print(std::mem::replace(&mut line, " ".to_owned()));
        }
        line.push(' ');
        line.push_str(name);
    }
    plan.print(line);
}

/// The `Get:` lines, at the speed of a decent connection, with apt's line
/// along the bottom saying what's coming in.
fn download(plan: &mut Plan, cx: &mut Cx, packages: &[&Package]) {
    let total: u64 = packages.iter().map(|p| p.size).sum::<u64>().max(1);
    let speed = cx.rng.range(6_000_000, 14_000_000);
    plan.status(Some(Status::Text("0% [Working]".to_owned())));
    plan.wait(cx.rng.range(200, 600));
    let mut done = 0;
    for (i, package) in packages.iter().enumerate() {
        let n = i + 1;
        plan.status(Some(Status::Text(format!("{}% [{n} {} {}]", done * 100 / total, package.name, size(package.size)))));
        plan.wait(cx.rng.range(10, 60) + package.size * 1000 / speed);
        let url = if package.pocket.starts_with("noble-security") { SECURITY } else { ARCHIVE };
        plan.print(format!(
            "Get:{n} {url} {} amd64 {} {} {} [{}]",
            package.pocket,
            package.name,
            package.arch,
            package.version,
            size(package.size)
        ));
        done += package.size;
    }
    let seconds = (total / speed).max(1);
    plan.status(None);
    plan.print(format!("Fetched {} in {seconds}s ({}/s)", size(total), size(total / seconds)));
}

/// The file apt hands dpkg: `.../03-libgomp1_14.2.0-4ubuntu2~24.04_amd64.deb ...`.
fn deb(index: usize, package: &Package) -> String {
    format!("{index:02}-{}_{}_{}.deb ...", package.name, package.version.replace(':', "%3a"), package.arch)
}

fn unpack_ms(cx: &mut Cx, package: &Package) -> u64 {
    cx.rng.range(40, 160) + (package.size / 15_000).min(2_500)
}

fn triggers(plan: &mut Plan, cx: &mut Cx, triggers: &[&str]) {
    for trigger in triggers {
        plan.wait(cx.rng.range(100, 300)).print(format!("Processing triggers for {trigger} ..."));
        let busy = if *trigger == MAN_DB { (1_500, 4_500) } else { (200, 900) };
        plan.wait(cx.rng.range(busy.0, busy.1));
    }
}

/// What needrestart says after apt, on WSL: there's no kernel to check.
fn needrestart(plan: &mut Plan, cx: &mut Cx) {
    plan.wait(cx.rng.range(200, 500)).print("Scanning processes...");
    plan.wait(cx.rng.range(900, 2_200)).print("Scanning linux images...");
    plan.wait(cx.rng.range(500, 1_200));
    for line in [
        "",
        "Failed to retrieve available kernel versions.",
        "",
        "Failed to check for processor microcode upgrades.",
        "",
        "No services need to be restarted.",
        "",
        "No containers need to be restarted.",
        "",
        "No user sessions are running outdated binaries.",
        "",
        "No VM guests are running outdated hypervisor (qemu) binaries on this host.",
    ] {
        plan.print(line);
    }
}

/// dpkg's share of the progress bar.
struct Dpkg {
    done: u32,
    total: u32,
}

impl Dpkg {
    fn new(packages: usize) -> Self {
        Self { done: 0, total: (packages as u32 * 2).max(1) }
    }

    fn step(&mut self, plan: &mut Plan) {
        self.done = (self.done + 1).min(self.total);
        plan.status(Some(Status::Progress(self.done * 100 / self.total)));
    }
}

/// `Mon 2026-11-02 14:21:07 CET`, as systemd writes the time.
fn systemd_time(time: DateTime<Local>) -> String {
    let zone = match time.offset().local_minus_utc() {
        0 => "UTC".to_owned(),
        3600 => "CET".to_owned(),
        7200 => "CEST".to_owned(),
        _ => time.format("%:z").to_string(),
    };
    format!("{} {zone}", time.format("%a %Y-%m-%d %H:%M:%S"))
}

/// `12s`, `1min 12s`, `5min` as systemd writes how long ago.
fn ago(elapsed: chrono::Duration) -> String {
    let seconds = elapsed.num_seconds().max(0);
    match (seconds / 60, seconds % 60) {
        (0, s) => format!("{s}s"),
        (m, 0) => format!("{m}min"),
        (m, s) if m < 10 => format!("{m}min {s}s"),
        (m, _) => format!("{m}min"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Feed, Machine};
    use super::*;

    fn feed() -> Feed {
        let machine = Machine { user: "peter".to_owned(), host: "DESKTOP-4KQ2".to_owned(), booted: Local::now() - chrono::Duration::days(3) };
        Feed::new(Box::new(Apt::new()), machine, 42)
    }

    #[test]
    fn it_starts_with_history_then_updates() {
        let mut feed = feed();
        feed.run_to(Duration::ZERO, Local::now());
        let text = feed.all_text();
        assert!(text.starts_with("peter@DESKTOP-4KQ2:~$ uptime\n"), "{text}");
        assert!(text.contains("/dev/sdd       1007G"));
        feed.run_to(Duration::from_secs(60), Local::now());
        let text = feed.all_text();
        assert!(text.contains("peter@DESKTOP-4KQ2:~$ sudo apt update\n[sudo] password for peter: \n"), "{text}");
        assert!(text.contains("Reading package lists... Done\nBuilding dependency tree... Done"));
        assert!(text.contains("packages can be upgraded."));
    }

    #[test]
    fn it_runs_for_hours_without_running_out() {
        let mut feed = feed();
        let (mut progress, mut purged, mut up_to_date) = (false, false, false);
        for seconds in (10..=2 * 60 * 60).step_by(10) {
            feed.run_to(Duration::from_secs(seconds), Local::now());
            progress |= matches!(feed.status(), Some(Status::Progress(_)));
            let text = feed.all_text();
            purged |= text.contains("sudo apt purge -y build-essential redis-server fail2ban prometheus-node-exporter")
                && text.contains("Purging configuration files for redis-server");
            up_to_date |= text.contains("All packages are up to date.");
        }
        assert!(progress, "dpkg's progress bar shows at some point");
        assert!(purged && up_to_date, "later passes purge and install again");
        assert!(feed.all_text().lines().count() <= super::super::SCROLLBACK + 1);
    }

    #[test]
    fn upgrades_and_installs_look_like_apt() {
        let mut feed = feed();
        let mut expected = vec![
            "Unpacking openssh-server (1:9.6p1-3ubuntu13.12) over (1:9.6p1-3ubuntu13.11) ...",
            "The following NEW packages will be installed:",
            "Get:1 http://archive.ubuntu.com/ubuntu noble-updates/main amd64 binutils-common amd64 2.42-4ubuntu2.5 [240 kB]",
            "Preparing to unpack .../11-libcrypt-dev_1%3a4.4.36-4build1_amd64.deb ...",
            "update-alternatives: using /usr/bin/gcc to provide /usr/bin/cc (cc) in auto mode",
            "No VM guests are running outdated hypervisor (qemu) binaries on this host.",
        ];
        // Each line has to show up while it's still in the scrollback.
        for seconds in 1..=600 {
            feed.run_to(Duration::from_secs(seconds), Local::now());
            let text = feed.all_text();
            expected.retain(|line| !text.contains(line));
        }
        assert!(expected.is_empty(), "never seen: {expected:?}");
    }

    #[test]
    fn times_read_like_systemd() {
        assert_eq!(ago(chrono::Duration::seconds(12)), "12s");
        assert_eq!(ago(chrono::Duration::seconds(72)), "1min 12s");
        assert_eq!(ago(chrono::Duration::seconds(600)), "10min");
    }
}
