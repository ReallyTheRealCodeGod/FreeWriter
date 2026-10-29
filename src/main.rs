//! Writer: a distraction-free writing app that works like an old typewriter.
//! Set a goal, and the page holds you until you reach it. See DESIGN.md.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod goal;
mod icon;
mod lock;
mod logbook;
mod menu;
mod page;
mod settings;

use eframe::egui;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `--windowed` runs in a normal window without the lock, for development.
    let windowed = args.iter().any(|arg| arg == "--windowed");
    // `--goal 500` or `--goal=25m` skips the start line, e.g. for a shortcut.
    let goal = option(&args, "--goal");
    // `--pages D:\Notes` keeps the pages somewhere other than Documents\Writing.
    let pages = option(&args, "--pages").map_or_else(page::pages_dir, Into::into);

    let mut viewport = egui::ViewportBuilder::default().with_title("Writer");
    if let Some(icon) = icon::icon() {
        viewport = viewport.with_icon(icon);
    }
    let viewport = if windowed {
        viewport.with_inner_size([900.0, 700.0])
    } else {
        // The page goes on the primary monitor; the others get blank covers.
        let primary = lock::monitors().primary;
        viewport.with_fullscreen(true).with_decorations(false).with_monitor(primary)
    };
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    // A Mac app gets a standard menu whose Quit (Cmd+Q) ends the app at once,
    // past the lock. Without it, Cmd+Q reaches egui, which asks the window to
    // close, and that the lock can refuse like any other close.
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_default_menu(false);
        }));
    }

    let result = eframe::run_native(
        "Writer",
        options,
        Box::new(move |cc| {
            load_typewriter_font(&cc.egui_ctx);
            let settings = settings::Settings::load();
            let logbook = settings::dir().map(logbook::Logbook::new);
            Ok(Box::new(app::WriterApp::new(settings, windowed, pages, logbook, goal)))
        }),
    );
    if let Err(error) = result {
        show_error(&format!("Writer couldn't start.\n\n{error}"));
    }
}

/// The value of `--name value` or `--name=value`.
fn option(args: &[String], name: &str) -> Option<String> {
    args.iter().enumerate().find_map(|(i, arg)| match arg.strip_prefix(name) {
        Some("") => args.get(i + 1).cloned(),
        Some(value) => value.strip_prefix('=').map(str::to_owned),
        None => None,
    })
}

/// The release build has no console, so startup errors go in a message box.
#[cfg(windows)]
fn show_error(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (text, title) = (wide(message), wide("Writer"));
    // SAFETY: both strings are NUL-terminated and outlive the call.
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR) };
}

#[cfg(not(windows))]
fn show_error(message: &str) {
    eprintln!("{message}");
}

/// Uses Courier New from the system's fonts (it ships with Windows and with
/// macOS). If it isn't there, egui's built-in monospace font is used instead.
fn load_typewriter_font(ctx: &egui::Context) {
    let Some(bytes) = typewriter_font_files().into_iter().find_map(|path| std::fs::read(path).ok()) else {
        return;
    };
    ctx.add_font(egui::FontInsert::new(
        "Courier New",
        egui::FontData::from_owned(bytes),
        vec![egui::InsertFontFamily {
            family: egui::FontFamily::Monospace,
            priority: egui::FontPriority::Highest,
        }],
    ));
}

/// Where Courier New is, first place first.
fn typewriter_font_files() -> Vec<std::path::PathBuf> {
    if cfg!(target_os = "macos") {
        vec![
            "/System/Library/Fonts/Supplemental/Courier New.ttf".into(),
            "/Library/Fonts/Courier New.ttf".into(),
        ]
    } else {
        let windows = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
        vec![std::path::Path::new(&windows).join("Fonts").join("cour.ttf")]
    }
}
