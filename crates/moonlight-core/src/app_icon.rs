//! Pulling a programme's own icon out of its executable.
//!
//! The macOS client asks `NSWorkspace` for an `NSImage` and is done. Windows has
//! no equivalent that hands back pixels: the shell answers with an `HICON`, a
//! GDI resource, and turning one into RGBA is four calls and a colour-order
//! swap. That is what this module is.
//!
//! Why bother, rather than drawing the programme's initial on a coloured tile:
//! a list of eight hundred installed programmes is scanned visually, and an icon
//! is what people actually recognise. A column of letters in five brand colours
//! looks tidy and reads as nothing.

/// A decoded icon, in the layout `iced::widget::image::Handle::from_rgba` wants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Rgba {
    /// Whether every pixel is fully transparent.
    ///
    /// Some executables carry an icon resource that decodes to nothing at all.
    /// Handing that to the UI produces a 42×42 hole in the row where the tile
    /// should be, which reads as a broken layout rather than as a missing icon,
    /// so the caller falls back to the lettered tile instead.
    pub fn is_blank(&self) -> bool {
        self.pixels.chunks_exact(4).all(|p| p[3] == 0)
    }
}

#[cfg(windows)]
pub use imp::load;

#[cfg(not(windows))]
pub fn load(_path: &str) -> Option<Rgba> {
    None
}

#[cfg(windows)]
mod imp {
    use super::Rgba;
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The icon for an executable, as RGBA.
    pub fn load(path: &str) -> Option<Rgba> {
        let wide_path = wide(path);
        let mut info = SHFILEINFOW::default();
        let result = unsafe {
            SHGetFileInfoW(
                PCWSTR(wide_path.as_ptr()),
                Default::default(),
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            )
        };
        if result == 0 || info.hIcon.is_invalid() {
            return None;
        }

        let icon = info.hIcon;
        let pixels = decode(icon);
        unsafe { let _ = DestroyIcon(icon); }
        pixels.filter(|rgba| !rgba.is_blank())
    }

    /// `HICON` → RGBA.
    ///
    /// The colour bitmap is read back as a 32-bit top-down DIB. GDI hands those
    /// over as **BGRA**, so the two outer channels are swapped on the way out.
    fn decode(icon: HICON) -> Option<Rgba> {
        let mut info = ICONINFO::default();
        unsafe { GetIconInfo(icon, &mut info) }.ok()?;

        // Both bitmaps are ours to free once we are done, on every path out.
        let colour = info.hbmColor;
        let mask = info.hbmMask;
        let result = read_bitmap(colour);
        unsafe {
            if !colour.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(colour.0));
            }
            if !mask.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(mask.0));
            }
        }
        result
    }

    fn read_bitmap(bitmap: HBITMAP) -> Option<Rgba> {
        if bitmap.is_invalid() {
            return None;
        }

        let mut shape = BITMAP::default();
        let written = unsafe {
            GetObjectW(
                HGDIOBJ(bitmap.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut shape as *mut _ as *mut std::ffi::c_void),
            )
        };
        if written == 0 || shape.bmWidth <= 0 || shape.bmHeight <= 0 {
            return None;
        }

        let width = shape.bmWidth as u32;
        let height = shape.bmHeight as u32;

        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: shape.bmWidth,
                // Negative height asks for a top-down DIB, which spares us
                // flipping the rows afterwards.
                biHeight: -shape.bmHeight,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return None;
        }
        let scanned = unsafe {
            GetDIBits(
                dc,
                bitmap,
                0,
                height,
                Some(pixels.as_mut_ptr() as *mut std::ffi::c_void),
                &mut header,
                DIB_RGB_COLORS,
            )
        };
        unsafe { let _ = DeleteDC(dc); }
        if scanned == 0 {
            return None;
        }

        // BGRA → RGBA.
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        Some(Rgba {
            width,
            height,
            pixels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_all_transparent_icon_is_blank() {
        let icon = Rgba {
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 0, 0, 255, 0, 0],
        };
        assert!(icon.is_blank());
    }

    #[test]
    fn one_opaque_pixel_is_enough_to_be_worth_drawing() {
        let icon = Rgba {
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 0, 0, 255, 0, 12],
        };
        assert!(!icon.is_blank());
    }

    #[test]
    #[cfg(windows)]
    fn a_real_system_executable_yields_an_icon() {
        // explorer.exe is present on every Windows install and always carries an
        // icon resource, so this exercises the whole HICON → RGBA path rather
        // than only the helpers around it.
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let path = format!(r"{root}\explorer.exe");
        let icon = load(&path).expect("explorer.exe should carry an icon");
        assert!(icon.width > 0 && icon.height > 0);
        assert_eq!(
            icon.pixels.len(),
            (icon.width * icon.height * 4) as usize,
            "the buffer must be exactly RGBA for the reported size"
        );
        assert!(!icon.is_blank());
    }
}
