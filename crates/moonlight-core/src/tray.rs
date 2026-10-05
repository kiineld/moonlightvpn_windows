//! The notification-area icon: the way back to a window that was closed, the
//! tray panel's anchor, and the source of the app's notifications.
//!
//! Raw `Shell_NotifyIcon` rather than a crate: the icon needs a window to
//! receive its clicks, and a hidden one on its own thread is all of it.
//! Its balloons are also how the expiry and traffic warnings reach the user —
//! Windows 10 and 11 show them as ordinary toasts, attributed to the app, with
//! no app identity to register first.
//!
//! The icon is the moon from the logo, drawn here rather than loaded: a
//! crescent in the taskbar's own ink while the tunnel is down, the full moon
//! in the brand's lime while it is up — so the taskbar answers "is it on"
//! without anything being opened, as the macOS client's menu-bar glyph does.
//!
//! A left click opens the panel; a right click opens a menu, as every other
//! icon in the notification area does, with the way to quit in it.
//!
//! Only the thread that made the icon reads its clicks; everything else here —
//! what it shows, a balloon, removing the icon — is a call any thread can
//! make, since the shell finds the icon by its window and id alone.

/// What the icon was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A click on the icon: open the panel, or put it away.
    Panel,
    /// From the icon's menu: bring the window forward.
    Open,
    /// From the icon's menu: connect, or disconnect.
    Toggle,
    /// From the icon's menu: leave.
    Quit,
}

/// What the icon shows and says, in the app's language.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Status {
    pub tooltip: String,
    /// The tunnel is up: the moon is full.
    pub connected: bool,
    /// The menu's lines. An empty `toggle` leaves that line out — there is
    /// nothing to connect to, or the tunnel is mid-change.
    pub open: String,
    pub toggle: String,
    pub quit: String,
}

/// A rectangle on the screen, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[cfg(windows)]
pub use imp::{anchor, notify, remove, spawn, update, work_area};

#[cfg(not(windows))]
pub fn spawn(_status: Status) -> Option<tokio::sync::mpsc::UnboundedReceiver<Event>> {
    None
}
#[cfg(not(windows))]
pub fn update(_status: Status) {}
#[cfg(not(windows))]
pub fn notify(_title: &str, _body: &str) {}
#[cfg(not(windows))]
pub fn remove() {}
#[cfg(not(windows))]
pub fn work_area() -> Option<(i32, i32, i32, i32, f32)> {
    None
}
#[cfg(not(windows))]
pub fn anchor() -> Option<Rect> {
    None
}

/// Copies `text` into a fixed UTF-16 field, cut to fit with its terminator.
#[cfg_attr(not(windows), allow(dead_code))]
fn fill(field: &mut [u16], text: &str) {
    let mut units: Vec<u16> = text.encode_utf16().take(field.len() - 1).collect();
    units.push(0);
    field[..units.len()].copy_from_slice(&units);
}

/// Where the panel goes: centred on the icon and just clear of it — above an
/// icon at the foot of the screen, below one at the top — and never off the
/// work area. Everything in physical pixels; returns the panel's top-left.
///
/// The icon is as often in the hidden-icons flyout as on the taskbar, and
/// then it is the flyout the panel has to clear, which is why this is placed
/// against the icon and not against the taskbar's edge.
pub fn place(icon: Rect, work: Rect, width: i32, height: i32, gap: i32) -> (i32, i32) {
    let centre = (icon.left + icon.right) / 2;
    let x = (centre - width / 2).clamp(
        work.left + gap,
        (work.right - width - gap).max(work.left + gap),
    );
    let above = (icon.top + icon.bottom) / 2 >= (work.top + work.bottom) / 2;
    let y = if above {
        icon.top.min(work.bottom) - gap - height
    } else {
        icon.bottom.max(work.top) + gap
    };
    let y = y.clamp(
        work.top + gap,
        (work.bottom - height - gap).max(work.top + gap),
    );
    (x, y)
}

/// The icon's pixels: `size` square, BGRA, straight alpha.
///
/// The moon from the logo. Down, a crescent in the taskbar's ink — white on
/// a dark taskbar, near-black on a light one. Up, the full moon in the
/// brand's lime; on a light taskbar it keeps a dark rim, because lime on
/// near-white has no edge of its own.
#[cfg_attr(not(windows), allow(dead_code))]
fn glyph(size: u32, connected: bool, light_taskbar: bool) -> Vec<u8> {
    const SAMPLES: u32 = 4;
    const LIME: [f32; 3] = [210.0, 255.0, 31.0];
    let ink: [f32; 3] = if light_taskbar {
        [16.0, 16.0, 16.0]
    } else {
        [255.0, 255.0, 255.0]
    };
    let side = size as f32;
    let (centre, radius) = (side / 2.0, side * 0.44);
    // The crescent's cut: a disc of the same size, up and to the right, as
    // the logo draws it.
    let cut = (centre + side * 0.27, centre - side * 0.21);
    let rim = if connected && light_taskbar {
        (side / 13.0).max(1.0)
    } else {
        0.0
    };

    let mut pixels = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            // How much of the pixel the moon covers, and how much of that is
            // inside its rim.
            let (mut outer, mut inner) = (0u32, 0u32);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    let from_centre = ((px - centre).powi(2) + (py - centre).powi(2)).sqrt();
                    if from_centre > radius {
                        continue;
                    }
                    if !connected {
                        let from_cut = ((px - cut.0).powi(2) + (py - cut.1).powi(2)).sqrt();
                        if from_cut < radius * 0.92 {
                            continue;
                        }
                    }
                    outer += 1;
                    if from_centre <= radius - rim {
                        inner += 1;
                    }
                }
            }
            if outer == 0 {
                continue;
            }
            let body = if connected { LIME } else { ink };
            let share = inner as f32 / outer as f32;
            let at = ((y * size + x) * 4) as usize;
            for channel in 0..3 {
                let value = body[channel] * share + ink[channel] * (1.0 - share);
                // BGRA: blue first.
                pixels[at + 2 - channel] = value.round() as u8;
            }
            pixels[at + 3] = (outer * 255 / (SAMPLES * SAMPLES)) as u8;
        }
    }
    pixels
}

#[cfg(windows)]
mod imp {
    use super::{fill, glyph, Event, Rect, Status};
    use std::sync::{Mutex, OnceLock};
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    use windows::Win32::UI::HiDpi::GetDpiForSystem;
    use windows::Win32::UI::Shell::{
        Shell_NotifyIconGetRect, Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP,
        NIF_TIP, NIIF_LARGE_ICON, NIIF_USER, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION,
        NIN_SELECT, NOTIFYICONDATAW, NOTIFYICONIDENTIFIER, NOTIFYICON_VERSION_4,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
        DestroyMenu, DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics, LoadIconW,
        LoadImageW, PostMessageW, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
        SystemParametersInfoW, TrackPopupMenu, HICON, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR,
        MF_SEPARATOR, MF_STRING, MSG, SM_CXSMICON, SPI_GETWORKAREA,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
        TPM_RIGHTBUTTON, WM_APP, WM_CONTEXTMENU, WM_NULL, WM_SETTINGCHANGE, WNDCLASSW,
        WS_EX_TOOLWINDOW, WS_POPUP,
    };

    const ICON_ID: u32 = 1;
    /// The icon chosen from the keyboard: `NIN_SELECT` with the key flag.
    const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
    const CALLBACK: u32 = WM_APP + 1;
    /// Any thread asks the icon's own to redraw it with this.
    const REFRESH: u32 = WM_APP + 2;

    const MENU_OPEN: usize = 1;
    const MENU_TOGGLE: usize = 2;
    const MENU_QUIT: usize = 3;

    /// Handles are kept as integers: `HWND` and `HICON` are raw pointers, which
    /// a `static` may not hold.
    struct Shared {
        window: isize,
        /// The executable's own icon, for the notifications.
        app_icon: isize,
        events: UnboundedSender<Event>,
        /// Explorer broadcasts this when it restarts, and every icon it showed
        /// is gone until its owner adds it again.
        taskbar_created: u32,
    }
    static SHARED: OnceLock<Shared> = OnceLock::new();
    static STATUS: Mutex<Status> = Mutex::new(Status {
        tooltip: String::new(),
        connected: false,
        open: String::new(),
        toggle: String::new(),
        quit: String::new(),
    });
    /// The moon as it was last drawn, and what it was drawn for — size, phase
    /// and the taskbar's theme — so it is redrawn only when one of them moves.
    static DRAWN: Mutex<Option<(Phase, isize)>> = Mutex::new(None);
    /// What a moon is drawn for: its size, whether it is full, and whether
    /// the taskbar is the light one.
    type Phase = (u32, bool, bool);

    fn status() -> Status {
        STATUS.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn data() -> Option<NOTIFYICONDATAW> {
        let shared = SHARED.get()?;
        Some(NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: HWND(shared.window as _),
            uID: ICON_ID,
            ..Default::default()
        })
    }

    /// Whether the taskbar is the light one. It has its own setting, apart
    /// from the one apps follow.
    fn taskbar_is_light() -> bool {
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("SystemUsesLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut value as *mut u32 as *mut _),
                Some(&mut size),
            )
            .is_ok()
                && value != 0
        }
    }

    /// The moon for the current state, drawn again only if it has changed.
    fn moon(connected: bool) -> HICON {
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as u32;
        let key = (size, connected, taskbar_is_light());
        let Ok(mut drawn) = DRAWN.lock() else {
            return HICON::default();
        };
        if let Some((was, icon)) = *drawn {
            if was == key {
                return HICON(icon as _);
            }
        }
        let pixels = glyph(size, key.1, key.2);
        // The mask is all "opaque": with 32-bit colour the alpha decides.
        let mask = vec![0u8; (size.div_ceil(16) * 2 * size) as usize];
        let icon = unsafe {
            CreateIcon(
                None,
                size as i32,
                size as i32,
                1,
                32,
                mask.as_ptr(),
                pixels.as_ptr(),
            )
        }
        .unwrap_or_default();
        if let Some((_, old)) = drawn.replace((key, icon.0 as isize)) {
            unsafe {
                let _ = DestroyIcon(HICON(old as _));
            }
        }
        icon
    }

    /// Puts the icon up, or brings what it shows up to date.
    fn show(message: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE) {
        let Some(mut data) = data() else { return };
        let status = status();
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = CALLBACK;
        data.hIcon = moon(status.connected);
        fill(&mut data.szTip, &status.tooltip);
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        unsafe {
            let _ = Shell_NotifyIconW(message, &data);
            if message == NIM_ADD {
                // Version 4 is what sends a keyboard selection the same way
                // as a mouse one, and the menu request as its own event.
                let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
            }
        }
    }

    /// Puts the icon up and starts reading its clicks. `None` when the shell
    /// would not have it — the app then runs as it always did, window only.
    pub fn spawn(status: Status) -> Option<UnboundedReceiver<Event>> {
        *STATUS.lock().ok()? = status;
        let (events, receiver) = unbounded_channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || unsafe { run(events, ready_tx) })
            .ok()?;
        ready_rx.recv().ok()?.then_some(receiver)
    }

    unsafe fn run(events: UnboundedSender<Event>, ready: std::sync::mpsc::Sender<bool>) {
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
        // A top-level window that is never shown, not a message-only one:
        // broadcasts — Explorer restarting, the taskbar changing theme — do
        // not reach message-only windows, and the icon has to hear both.
        let Ok(window) = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("moonlight-tray"),
            w!("moonlight"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(module.into()),
            None,
        ) else {
            let _ = ready.send(false);
            return;
        };

        // The executable's own icon (winresource embeds it as resource 1),
        // which the notifications carry.
        let app_icon = LoadImageW(
            Some(module.into()),
            PCWSTR(1 as _),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTCOLOR,
        )
        .map(|handle| HICON(handle.0))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default();

        let _ = SHARED.set(Shared {
            window: window.0 as isize,
            app_icon: app_icon.0 as isize,
            events,
            taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
        });
        show(NIM_ADD);
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
                // Version 4: the event is the low word of `lParam`.
                match (lparam.0 & 0xFFFF) as u32 {
                    NIN_SELECT | NIN_KEYSELECT => {
                        let _ = shared.events.send(Event::Panel);
                    }
                    WM_CONTEXTMENU => menu(window, shared),
                    _ => {}
                }
                return LRESULT(0);
            }
            if message == shared.taskbar_created {
                show(NIM_ADD);
                return LRESULT(0);
            }
            // The taskbar's theme is one of the settings this announces, and
            // the moon is drawn in its ink.
            if message == REFRESH || message == WM_SETTINGCHANGE {
                show(NIM_MODIFY);
                return LRESULT(0);
            }
        }
        DefWindowProcW(window, message, wparam, lparam)
    }

    /// The icon's own menu, at the pointer.
    unsafe fn menu(window: HWND, shared: &Shared) {
        let status = status();
        let Ok(menu) = CreatePopupMenu() else { return };
        let line = |id: usize, label: &str| {
            if label.is_empty() {
                return;
            }
            let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            let _ = AppendMenuW(menu, MF_STRING, id, PCWSTR(wide.as_ptr()));
        };
        line(MENU_OPEN, &status.open);
        line(MENU_TOGGLE, &status.toggle);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        line(MENU_QUIT, &status.quit);

        let mut at = POINT::default();
        let _ = GetCursorPos(&mut at);
        // The menu's owner has to be the foreground window, or a click
        // elsewhere does not close the menu; the null message afterwards is
        // the other half of the same documented fix.
        let _ = SetForegroundWindow(window);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            at.x,
            at.y,
            None,
            window,
            None,
        );
        let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        let event = match chosen.0 as usize {
            MENU_OPEN => Event::Open,
            MENU_TOGGLE => Event::Toggle,
            MENU_QUIT => Event::Quit,
            _ => return,
        };
        let _ = shared.events.send(event);
    }

    /// What the icon shows and says from now on.
    pub fn update(status: Status) {
        let Ok(mut current) = STATUS.lock() else {
            return;
        };
        if *current == status {
            return;
        }
        *current = status;
        drop(current);
        // Redrawn by the icon's own thread, which is the one the moon's
        // handle belongs to.
        if let Some(shared) = SHARED.get() {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(shared.window as _)),
                    REFRESH,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
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
        data.hBalloonIcon = HICON(shared.app_icon as _);
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

    /// Where the icon is on the screen — on the taskbar, or in the
    /// hidden-icons flyout while that is open. Failing that, the pointer,
    /// which has just clicked it.
    pub fn anchor() -> Option<Rect> {
        let shared = SHARED.get()?;
        let identifier = NOTIFYICONIDENTIFIER {
            cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: HWND(shared.window as _),
            uID: ICON_ID,
            ..Default::default()
        };
        unsafe {
            if let Ok(rect) = Shell_NotifyIconGetRect(&identifier) {
                if rect.right > rect.left {
                    return Some(Rect {
                        left: rect.left,
                        top: rect.top,
                        right: rect.right,
                        bottom: rect.bottom,
                    });
                }
            }
            let mut at = POINT::default();
            GetCursorPos(&mut at).ok()?;
            Some(Rect {
                left: at.x,
                top: at.y,
                right: at.x,
                bottom: at.y,
            })
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
    use super::{fill, glyph, place, Rect};

    const WORK: Rect = Rect {
        left: 0,
        top: 0,
        right: 2560,
        bottom: 1392,
    };

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

    #[test]
    fn the_panel_sits_over_an_icon_on_the_taskbar_and_centred_on_it() {
        // The icon's cell runs the height of the taskbar, below the work area.
        let icon = Rect {
            left: 2000,
            top: 1392,
            right: 2032,
            bottom: 1440,
        };
        let (x, y) = place(icon, WORK, 384, 620, 12);
        assert_eq!(x + 192, 2016, "centred on the icon");
        assert_eq!(y + 620, 1392 - 12, "just above the taskbar");
    }

    #[test]
    fn the_panel_clears_the_hidden_icons_flyout() {
        // In the flyout the icon is well above the taskbar, and the panel has
        // to sit above the flyout, not over it.
        let icon = Rect {
            left: 2211,
            top: 1255,
            right: 2251,
            bottom: 1295,
        };
        let (_, y) = place(icon, WORK, 384, 620, 12);
        assert!(y + 620 <= 1255 - 12);
    }

    #[test]
    fn the_panel_never_leaves_the_work_area() {
        // An icon in the corner: centred on it the panel would hang off the
        // screen's edge.
        let icon = Rect {
            left: 2530,
            top: 1392,
            right: 2560,
            bottom: 1440,
        };
        let (x, _) = place(icon, WORK, 384, 620, 12);
        assert_eq!(x + 384, 2560 - 12);
        // A taskbar at the top: the panel opens downwards.
        let top = Rect {
            left: 2000,
            top: 0,
            right: 2032,
            bottom: 48,
        };
        let work = Rect { top: 48, ..WORK };
        let (_, y) = place(top, work, 384, 620, 12);
        assert_eq!(y, 48 + 12);
        // A screen shorter than the panel: its head stays on screen.
        let short = Rect {
            bottom: 500,
            ..WORK
        };
        let icon = Rect {
            left: 100,
            top: 500,
            right: 132,
            bottom: 548,
        };
        let (x, y) = place(icon, short, 384, 620, 12);
        assert_eq!((x, y), (12, 12));
    }

    #[test]
    fn the_moon_is_a_crescent_down_and_full_up() {
        let size = 32u32;
        let covered = |pixels: &[u8]| pixels.chunks(4).filter(|p| p[3] > 127).count();
        let down = glyph(size, false, false);
        let up = glyph(size, true, false);
        assert_eq!(down.len(), (size * size * 4) as usize);
        // The cut takes a good part of the disc away.
        assert!(covered(&down) * 10 < covered(&up) * 8);
        // The upper right is the cut; the lower left is lit.
        let at = |pixels: &[u8], x: u32, y: u32| pixels[((y * size + x) * 4 + 3) as usize];
        assert_eq!(at(&down, 22, 9), 0);
        assert_eq!(at(&down, 8, 22), 255);
        assert_eq!(at(&up, 22, 9), 255);
        // The corners are outside the moon in either phase.
        assert_eq!(at(&up, 0, 0), 0);
    }

    #[test]
    fn the_moon_is_lime_up_and_the_taskbars_ink_down() {
        let middle = |pixels: &[u8]| {
            let at = ((16 * 32 + 14) * 4) as usize;
            // BGRA.
            (pixels[at + 2], pixels[at + 1], pixels[at])
        };
        assert_eq!(middle(&glyph(32, true, false)), (210, 255, 31));
        assert_eq!(middle(&glyph(32, true, true)), (210, 255, 31));
        // Lower left of the crescent, where it is lit.
        let lit = |pixels: &[u8]| {
            let at = ((22 * 32 + 8) * 4) as usize;
            (pixels[at + 2], pixels[at + 1], pixels[at])
        };
        assert_eq!(lit(&glyph(32, false, false)), (255, 255, 255));
        assert_eq!(lit(&glyph(32, false, true)), (16, 16, 16));
        // On a light taskbar the full moon keeps a dark rim.
        let edge = |pixels: &[u8]| {
            let at = ((16 * 32 + 2) * 4) as usize;
            pixels[at + 1]
        };
        assert!(edge(&glyph(32, true, true)) < 100);
        assert_eq!(edge(&glyph(32, true, false)), 255);
    }
}
