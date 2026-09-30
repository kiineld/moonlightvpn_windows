//! Windows 11's own material behind the window: Mica for the main window,
//! Acrylic for the tray panel — the Windows counterpart of the macOS client's
//! Liquid Glass.
//!
//! DWM draws it; the app only asks, and then paints its canvas translucent so
//! the material shows through. The request is its own feature check: Windows 10
//! and early 11 have no system backdrop, the call fails there, and the caller
//! keeps its surfaces opaque.

/// Which material to ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    /// The main window: the desktop, blurred and tinted, sampled once.
    Mica,
    /// A transient surface like the tray panel: a live blur of what is behind.
    Acrylic,
}

/// Asks DWM for `material` behind `hwnd`, in the dark or light variant.
/// Returns whether the system took it.
#[cfg(windows)]
pub fn apply(hwnd: isize, material: Material, dark: bool) -> bool {
    use windows::core::BOOL;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMSBT_MAINWINDOW,
        DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    use windows::Win32::UI::Controls::MARGINS;

    let hwnd = HWND(hwnd as _);
    let kind = match material {
        Material::Mica => DWMSBT_MAINWINDOW,
        Material::Acrylic => DWMSBT_TRANSIENTWINDOW,
    };
    let dark = BOOL::from(dark);
    let corners = DWMWCP_ROUND;
    unsafe {
        // The material follows the window's own light or dark setting, not the
        // system's, so the two are told the same thing.
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as _,
            std::mem::size_of::<BOOL>() as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corners as *const _ as _,
            std::mem::size_of_val(&corners) as u32,
        );
        // The frame reaches over the whole client area, which is what lets the
        // material show wherever the app paints translucent.
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &kind as *const _ as _,
            std::mem::size_of_val(&kind) as u32,
        )
        .is_ok()
    }
}

#[cfg(not(windows))]
pub fn apply(_hwnd: isize, _material: Material, _dark: bool) -> bool {
    false
}
