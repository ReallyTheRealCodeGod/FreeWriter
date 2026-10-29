/// Is Task Manager the window in front?
///
/// The lock steps aside for Task Manager, so Ctrl+Alt+Del or Ctrl+Shift+Esc
/// is always a way out.
#[cfg(windows)]
pub fn task_manager_in_front() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    // SAFETY: plain Win32 calls with valid out-pointers; the process handle is
    // closed before returning.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        if pid == 0 {
            return false;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return false;
        }
        let mut name = [0u16; 512];
        let mut len = name.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, name.as_mut_ptr(), &mut len);
        CloseHandle(process);
        if ok == 0 {
            return false;
        }
        let path = String::from_utf16_lossy(&name[..len as usize]);
        path.rsplit('\\')
            .next()
            .is_some_and(|exe| exe.eq_ignore_ascii_case("Taskmgr.exe"))
    }
}

/// On a Mac, the way out is Activity Monitor or the Force Quit window
/// (Cmd+Option+Esc), which belongs to loginwindow. The lock steps aside for
/// either, the way it does for Task Manager on Windows.
#[cfg(target_os = "macos")]
pub fn task_manager_in_front() -> bool {
    use objc2_app_kit::NSWorkspace;

    let Some(front) = NSWorkspace::sharedWorkspace().frontmostApplication() else { return false };
    let Some(id) = front.bundleIdentifier() else { return false };
    matches!(id.to_string().as_str(), "com.apple.ActivityMonitor" | "com.apple.loginwindow")
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn task_manager_in_front() -> bool {
    false
}

/// The monitors: how many, and which is the primary one, where the page goes.
/// They're numbered in the order Windows lists them, the same order egui's
/// `ViewportBuilder::with_monitor` counts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Monitors {
    pub count: usize,
    pub primary: usize,
}

impl Monitors {
    /// The monitors that get a blank cover during a locked session: all but
    /// the page's.
    pub fn others(self) -> impl Iterator<Item = usize> {
        (0..self.count).filter(move |&index| index != self.primary)
    }
}

#[cfg(windows)]
pub fn monitors() -> Monitors {
    use windows_sys::Win32::Foundation::{LPARAM, RECT, TRUE};
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
    use windows_sys::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
    use windows_sys::core::BOOL;

    unsafe extern "system" fn add(monitor: HMONITOR, _: HDC, _: *mut RECT, list: LPARAM) -> BOOL {
        // SAFETY: `list` is the `Vec` passed to EnumDisplayMonitors below,
        // which only calls this while that call is running.
        unsafe { (*(list as *mut Vec<HMONITOR>)).push(monitor) };
        TRUE
    }
    let mut list: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only touches `list`, which outlives the call.
    unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(add), &raw mut list as LPARAM);
    }
    let is_primary = |&monitor: &HMONITOR| {
        // SAFETY: MONITORINFO is plain data; cbSize says which version it is.
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = size_of::<MONITORINFO>() as u32;
        unsafe { GetMonitorInfoW(monitor, &mut info) != 0 && info.dwFlags & MONITORINFOF_PRIMARY != 0 }
    };
    let primary = list.iter().position(is_primary).unwrap_or(0);
    Monitors { count: list.len().max(1), primary }
}

#[cfg(not(windows))]
pub fn monitors() -> Monitors {
    Monitors { count: 1, primary: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_monitor_list_makes_sense() {
        let monitors = monitors();
        assert!(monitors.count >= 1 && monitors.primary < monitors.count);
        assert_eq!(monitors.others().count(), monitors.count - 1);
        assert!(monitors.others().all(|index| index != monitors.primary));
    }
}
