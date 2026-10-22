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

#[cfg(not(windows))]
pub fn task_manager_in_front() -> bool {
    false
}
