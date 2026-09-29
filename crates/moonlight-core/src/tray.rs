//! The notification-area icon: the way back to a window that was closed, the
//! tray panel's anchor, and the source of the app's notifications.
//!
//! Raw `Shell_NotifyIcon` rather than a crate: the icon needs a window to
//! receive its clicks, and a message-only one on its own thread is all of it.
//! Its balloons are also how the expiry and traffic warnings reach the user —
//! Windows 10 and 11 show them as ordinary toasts, attributed to the app, with
//! no app identity to register first.
//!
//! Only the thread that made the icon reads its clicks; everything else here —
//! the tooltip, a balloon, removing the icon — is a call any thread can make,
//! since the shell finds the icon by its window and id alone.

/// A click on the icon, in physical screen pixels: where the panel opens from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    pub x: i32,
    pub y: i32,
}

#[cfg(windows)]
pub use imp::{notify, remove, set_tooltip, spawn, work_area};

#[cfg(not(windows))]
pub fn spawn(_tooltip: &str) -> Option<tokio::sync::mpsc::UnboundedReceiver<Click>> {
    None
}
#[cfg(not(windows))]
pub fn set_tooltip(_text: &str) {}
#[cfg(not(windows))]
pub fn notify(_title: &str, _body: &str) {}
#[cfg(not(windows))]
pub fn remove() {}
#[cfg(not(windows))]
pub fn work_area() -> Option<(i32, i32, i32, i32, f32)> {
    None
}

/// Copies `text` into a fixed UTF-16 field, cut to fit with its terminator.
#[cfg_attr(not(windows), allow(dead_code))]
fn fill(field: &mut [u16], text: &str) {
    let mut units: Vec<u16> = text.encode_utf16().take(field.len() - 1).collect();
    units.push(0);
    field[..units.len()].copy_from_slice(&units);
}

#[cfg(windows)]
mod imp {
    use super::{fill, Click};
    use std::sync::OnceLock;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::HiDpi::GetDpiForSystem;
    use windows::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_LARGE_ICON,
        NIIF_USER, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW,
        NOTIFYICON_VERSION_4,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetSystemMetrics,
        LoadIconW, LoadImageW, RegisterClassW, RegisterWindowMessageW, SystemParametersInfoW,
        HICON, HWND_MESSAGE, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, MSG, SM_CXSMICON,
        SM_CYSMICON, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WINDOW_EX_STYLE,
        WINDOW_STYLE, WM_APP, WM_CONTEXTMENU, WNDCLASSW,
    };

    const ICON_ID: u32 = 1;
    const CALLBACK: u32 = WM_APP + 1;

    /// Handles are kept as integers: `HWND` and `HICON` are raw pointers, which
    /// a `static` may not hold.
    struct Shared {
        window: isize,
        icon: isize,
        clicks: UnboundedSender<Click>,
        /// Explorer broadcasts this when it restarts, and every icon it showed
        /// is gone until its owner adds it again.
        taskbar_created: u32,
    }
    static SHARED: OnceLock<Shared> = OnceLock::new();
    static TOOLTIP: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

    fn data() -> Option<NOTIFYICONDATAW> {
        let shared = SHARED.get()?;
        Some(NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: HWND(shared.window as _),
            uID: ICON_ID,
            ..Default::default()
        })
    }

    fn add() {
        let Some(shared) = SHARED.get() else { return };
        let Some(mut data) = data() else { return };
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = CALLBACK;
        data.hIcon = HICON(shared.icon as _);
        fill(&mut data.szTip, &TOOLTIP.lock().expect("tooltip"));
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &data);
            // Version 4 is what puts the click position in `wParam` and sends
            // a keyboard selection the same way as a mouse one.
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
        }
    }

    /// Puts the icon up and starts reading its clicks. `None` when the shell
    /// would not have it — the app then runs as it always did, window only.
    pub fn spawn(tooltip: &str) -> Option<UnboundedReceiver<Click>> {
        *TOOLTIP.lock().expect("tooltip") = tooltip.to_string();
        let (clicks, receiver) = unbounded_channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || unsafe { run(clicks, ready_tx) })
            .ok()?;
        ready_rx.recv().ok()?.then_some(receiver)
    }

    unsafe fn run(clicks: UnboundedSender<Click>, ready: std::sync::mpsc::Sender<bool>) {
        let Ok(module) = GetModuleHandleW(PCWSTR::null()) else {
            let _ = ready.send(false);
            return;
        };
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: module.into(),
            lpszClassName: w!("moonlight-tray"),
            ..Default::default()
        };
        RegisterClassW(&class);
        let Ok(window) = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("moonlight-tray"),
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(module.into()),
            None,
        ) else {
            let _ = ready.send(false);
            return;
        };

        // The executable's own icon (winresource embeds it as resource 1), at
        // the size the notification area draws.
        let icon = LoadImageW(
            Some(module.into()),
            PCWSTR(1 as _),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        )
        .map(|handle| HICON(handle.0))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default();

        let _ = SHARED.set(Shared {
            window: window.0 as isize,
            icon: icon.0 as isize,
            clicks,
            taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
        });
        add();
        let _ = ready.send(true);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            DispatchMessageW(&message);
        }
    }

    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if let Some(shared) = SHARED.get() {
            if message == CALLBACK {
                // Version 4: the event is the low word of `lParam`, and the
                // anchor point is packed into `wParam`.
                let event = (lparam.0 & 0xFFFF) as u32;
                if event == NIN_SELECT || event == WM_CONTEXTMENU {
                    let x = (wparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let _ = shared.clicks.send(Click { x, y });
                }
                return LRESULT(0);
            }
            if message == shared.taskbar_created {
                add();
                return LRESULT(0);
            }
        }
        DefWindowProcW(window, message, wparam, lparam)
    }

    pub fn set_tooltip(text: &str) {
        *TOOLTIP.lock().expect("tooltip") = text.to_string();
        let Some(mut data) = data() else { return };
        data.uFlags = NIF_TIP | NIF_SHOWTIP;
        fill(&mut data.szTip, text);
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
        }
    }

    /// A notification, shown by Windows as a toast from this app.
    pub fn notify(title: &str, body: &str) {
        let Some(shared) = SHARED.get() else { return };
        let Some(mut data) = data() else { return };
        data.uFlags = NIF_INFO;
        fill(&mut data.szInfoTitle, title);
        fill(&mut data.szInfo, body);
        data.dwInfoFlags = NIIF_USER | NIIF_LARGE_ICON;
        data.hBalloonIcon = HICON(shared.icon as _);
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
        }
    }

    /// Takes the icon down. Left up, it lingers until the pointer passes over
    /// it after the process has gone.
    pub fn remove() {
        let Some(data) = data() else { return };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
    }

    /// The desktop minus the taskbar, in physical pixels, and the scale that
    /// turns those into the logical units windows are placed in.
    pub fn work_area() -> Option<(i32, i32, i32, i32, f32)> {
        let mut area = RECT::default();
        unsafe {
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut area as *mut RECT as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
            .ok()?;
            let scale = GetDpiForSystem() as f32 / 96.0;
            Some((area.left, area.top, area.right, area.bottom, scale))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fill;

    #[test]
    fn text_is_cut_to_the_field_and_always_terminated() {
        let mut field = [0xFFFFu16; 8];
        fill(&mut field, "moonlight · Подключено");
        assert_eq!(field[7], 0, "the last unit is the terminator");
        assert_eq!(String::from_utf16_lossy(&field[..7]), "moonlig");

        let mut roomy = [0xFFFFu16; 16];
        fill(&mut roomy, "ok");
        assert_eq!(&roomy[..3], &[b'o' as u16, b'k' as u16, 0]);
    }
}
