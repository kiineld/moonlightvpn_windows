//! The named pipe the service listens on, and the DACL that decides who may
//! open it.
//!
//! ## The DACL is the authentication
//!
//! There is no token, no shared secret and no handshake in this protocol,
//! because on Windows there does not need to be one: the pipe is created with a
//! security descriptor that grants access to exactly two principals, and the
//! kernel enforces it on `CreateFile`. A caller who cannot open the pipe never
//! reaches a single line of the code below.
//!
//! ```text
//! D:(A;;GA;;;BA)(A;;GA;;;SY)
//!    │    │   │
//!    │    │   └─ BA = BUILTIN\Administrators, SY = NT AUTHORITY\SYSTEM
//!    │    └───── GA = GENERIC_ALL
//!    └────────── A  = allow
//! ```
//!
//! `D:` with no inheritance flags means the DACL is exactly these two entries —
//! no implicit Everyone, no inherited grants. This is the counterpart of the
//! macOS client's `0660 root:admin` socket, and it buys the same thing: callers
//! are exactly the accounts that could already elevate, so the user is spared a
//! prompt per connect. It is deliberately *not* a boundary against an
//! administrator, and nothing here pretends otherwise.

use std::io::{BufRead, BufReader, Read, Write};

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL, INVALID_HANDLE_VALUE};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_WAIT,
};

use moonlight_core::helper::{
    decode_request, encode_response, Response, MAX_REQUEST_BYTES, PIPE_NAME,
};

/// Administrators and SYSTEM, and nobody else.
const SDDL: &str = "D:(A;;GA;;;BA)(A;;GA;;;SY)";

const BUFFER: u32 = 64 * 1024;

/// Owns the converted security descriptor for as long as the attributes point
/// at it — `SECURITY_ATTRIBUTES` holds a raw pointer, so the descriptor must not
/// be freed before `CreateNamedPipeW` has read it.
struct Descriptor(PSECURITY_DESCRIPTOR);

impl Descriptor {
    fn from_sddl(sddl: &str) -> Option<Descriptor> {
        let text = HSTRING::from(sddl);
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(text.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
            .ok()?;
        }
        Some(Descriptor(descriptor))
    }

    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0 .0,
            bInheritHandle: false.into(),
        }
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0 .0)));
        }
    }
}

/// Serves requests until `should_stop` says otherwise.
///
/// One connection at a time, deliberately: the service has exactly one core to
/// manage, and concurrent callers racing to start and stop it is a state machine
/// nobody needs. A second caller waits.
pub fn serve<F, S>(mut handle_request: F, should_stop: S)
where
    F: FnMut(&moonlight_core::helper::Request) -> Response,
    S: Fn() -> bool,
{
    let Some(descriptor) = Descriptor::from_sddl(SDDL) else {
        // Without the descriptor the pipe would be created with the default
        // DACL, which grants far more than intended. Refusing to listen at all
        // is the only safe response.
        return;
    };

    let mut first = true;
    while !should_stop() {
        let attributes = descriptor.attributes();
        // FIRST_PIPE_INSTANCE on the first pass: if another process already
        // holds this name, that is a name-squatting attempt and the service
        // must not join it as a second instance.
        let flags = if first {
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE
        } else {
            PIPE_ACCESS_DUPLEX
        };

        let name = HSTRING::from(PIPE_NAME);
        let pipe = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                flags,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1, // one instance: one core to manage
                BUFFER,
                BUFFER,
                0,
                Some(&attributes),
            )
        };
        first = false;

        if pipe == INVALID_HANDLE_VALUE {
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }

        let connected = unsafe { ConnectNamedPipe(pipe, None) }.is_ok();
        if connected {
            handle_connection(pipe, &mut handle_request);
        }

        unsafe {
            let _ = DisconnectNamedPipe(pipe);
            let _ = CloseHandle(pipe);
        }
    }
}

fn handle_connection<F>(pipe: HANDLE, handle_request: &mut F)
where
    F: FnMut(&moonlight_core::helper::Request) -> Response,
{
    let mut file = unsafe {
        use std::os::windows::io::FromRawHandle;
        std::fs::File::from_raw_handle(pipe.0 as *mut _)
    };

    let mut line = String::new();
    // The read is bounded: an unauthenticated writer must not be able to make a
    // LocalSystem service allocate without limit. `MAX_REQUEST_BYTES` is
    // checked again by `validate`, but not before this much has been read.
    let mut reader = BufReader::new(&mut file).take(MAX_REQUEST_BYTES as u64 + 1);
    let read = reader.read_line(&mut line);

    let response = match read {
        Err(error) => Response::Error {
            message: format!("Could not read the request: {error}"),
        },
        Ok(0) => {
            // The caller opened the pipe and said nothing.
            std::mem::forget(file);
            return;
        }
        Ok(_) => match decode_request(&line) {
            None => Response::Error {
                message: "Unrecognised request".to_string(),
            },
            Some(request) => handle_request(&request),
        },
    };

    let _ = file.write_all(encode_response(&response).as_bytes());
    let _ = file.flush();

    // The handle is closed by the caller of `handle_connection`, not by
    // dropping the File — doing both is a double close.
    std::mem::forget(file);
}
