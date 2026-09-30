//! Stamps the Windows resources onto `moonlight.exe`.
//!
//! `winresource` was already a build-dependency, but there was no build script
//! for it to run — so the executable shipped with no icon at all and Windows
//! drew the generic one in the taskbar, in Explorer and on the shortcut the
//! installer creates.
//!
//! The icon is committed rather than generated here: drawing it needs GDI+, and
//! a build that reaches for a drawing library is a build that breaks on the
//! first machine without one. `scripts/make-icon.ps1` regenerates it when the
//! mark changes.

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/moonlight.ico");

        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/moonlight.ico");
        // What Task Manager and the file's properties show — the brand, in
        // lower case as everywhere else it is seen.
        resource.set("FileDescription", "moonlight");
        resource.set("ProductName", "moonlight");
        resource.set("CompanyName", "moonlight");
        resource.set("LegalCopyright", "MIT licensed");

        // A missing resource compiler must not fail the build: the binary is
        // perfectly good without an icon, and this runs on machines that have
        // no Windows SDK on PATH.
        if let Err(error) = resource.compile() {
            println!("cargo:warning=could not embed the icon: {error}");
        }
    }
}
