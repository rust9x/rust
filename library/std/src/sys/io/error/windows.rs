use crate::sys::pal::{api, c};
use crate::{fmt, io, ptr};

#[cfg(test)]
mod tests;

pub fn errno() -> i32 {
    api::get_last_error().code as i32
}

#[inline]
pub fn is_interrupted(_errno: i32) -> bool {
    false
}

#[cfg_attr(feature = "optimize_for_size", optimize(size))]
pub fn decode_error_kind(errno: i32) -> io::ErrorKind {
    use io::ErrorKind::*;

    const fn e(code: u32, kind: io::ErrorKind) -> (u16, io::ErrorKind) {
        assert!(code <= u16::MAX as u32);
        (code as u16, kind)
    }

    // Sorted so the common low codes are found first.
    const TABLE: [(u16, io::ErrorKind); 68] = {
        let t = [
            e(c::ERROR_FILE_NOT_FOUND, NotFound),
            e(c::ERROR_PATH_NOT_FOUND, NotFound),
            e(c::ERROR_TOO_MANY_OPEN_FILES, TooManyOpenFiles),
            e(c::ERROR_ACCESS_DENIED, PermissionDenied),
            e(c::ERROR_NOT_ENOUGH_MEMORY, OutOfMemory),
            e(c::ERROR_OUTOFMEMORY, OutOfMemory),
            e(c::ERROR_INVALID_DRIVE, NotFound),
            e(c::ERROR_NOT_SAME_DEVICE, CrossesDevices),
            e(c::ERROR_WRITE_PROTECT, ReadOnlyFilesystem),
            e(c::ERROR_HANDLE_DISK_FULL, StorageFull),
            e(c::ERROR_BAD_NETPATH, NotFound),
            e(c::ERROR_BAD_NET_NAME, NotFound),
            e(c::ERROR_FILE_EXISTS, AlreadyExists),
            e(c::ERROR_INVALID_PARAMETER, InvalidInput),
            e(c::ERROR_BROKEN_PIPE, BrokenPipe),
            e(c::ERROR_DISK_FULL, StorageFull),
            e(c::ERROR_CALL_NOT_IMPLEMENTED, Unsupported),
            e(c::ERROR_SEM_TIMEOUT, TimedOut),
            e(c::ERROR_INVALID_NAME, InvalidFilename),
            e(c::ERROR_NEGATIVE_SEEK, InvalidInput),
            e(c::ERROR_SEEK_ON_DEVICE, NotSeekable),
            e(c::ERROR_DIR_NOT_EMPTY, DirectoryNotEmpty),
            e(c::ERROR_BAD_PATHNAME, InvalidFilename),
            e(c::ERROR_BUSY, ResourceBusy),
            e(c::ERROR_ALREADY_EXISTS, AlreadyExists),
            e(c::ERROR_FILENAME_EXCED_RANGE, InvalidFilename),
            e(c::ERROR_FILE_TOO_LARGE, FileTooLarge),
            e(c::ERROR_NO_DATA, BrokenPipe),
            e(c::WAIT_TIMEOUT, TimedOut),
            e(c::ERROR_DIRECTORY, NotADirectory),
            e(c::ERROR_DIRECTORY_NOT_SUPPORTED, IsADirectory),
            e(c::ERROR_DRIVER_CANCEL_TIMEOUT, TimedOut),
            e(c::ERROR_OPERATION_ABORTED, TimedOut),
            e(c::ERROR_SERVICE_REQUEST_TIMEOUT, TimedOut),
            e(c::ERROR_IO_DEVICE, InputOutputError),
            e(c::ERROR_COUNTER_TIMEOUT, TimedOut),
            e(c::ERROR_POSSIBLE_DEADLOCK, Deadlock),
            e(c::ERROR_TOO_MANY_LINKS, TooManyLinks),
            e(c::ERROR_NETWORK_UNREACHABLE, NetworkUnreachable),
            e(c::ERROR_HOST_UNREACHABLE, HostUnreachable),
            e(c::ERROR_DISK_QUOTA_EXCEEDED, QuotaExceeded),
            e(c::ERROR_TIMEOUT, TimedOut),
            e(c::ERROR_CANT_RESOLVE_FILENAME, FilesystemLoop),
            e(c::ERROR_RESOURCE_CALL_TIMED_OUT, TimedOut),
            e(c::ERROR_CTX_MODEM_RESPONSE_TIMEOUT, TimedOut),
            e(c::ERROR_CTX_CLIENT_QUERY_TIMEOUT, TimedOut),
            e(c::FRS_ERR_SYSVOL_POPULATE_TIMEOUT, TimedOut),
            e(c::ERROR_DS_TIMELIMIT_EXCEEDED, TimedOut),
            e(c::DNS_ERROR_RECORD_TIMED_OUT, TimedOut),
            e(c::WSAEACCES as u32, PermissionDenied),
            e(c::WSAEINVAL as u32, InvalidInput),
            e(c::WSAEMFILE as u32, TooManyOpenFiles),
            e(c::WSAEWOULDBLOCK as u32, WouldBlock),
            e(c::WSAEADDRINUSE as u32, AddrInUse),
            e(c::WSAEADDRNOTAVAIL as u32, AddrNotAvailable),
            e(c::WSAENETDOWN as u32, NetworkDown),
            e(c::WSAENETUNREACH as u32, NetworkUnreachable),
            e(c::WSAECONNABORTED as u32, ConnectionAborted),
            e(c::WSAECONNRESET as u32, ConnectionReset),
            e(c::WSAENOTCONN as u32, NotConnected),
            e(c::WSAESHUTDOWN as u32, BrokenPipe),
            e(c::WSAETIMEDOUT as u32, TimedOut),
            e(c::WSAECONNREFUSED as u32, ConnectionRefused),
            e(c::WSAEHOSTUNREACH as u32, HostUnreachable),
            e(c::WSAEDQUOT as u32, QuotaExceeded),
            e(c::ERROR_IPSEC_IKE_TIMED_OUT, TimedOut),
            e(c::ERROR_RUNLEVEL_SWITCH_TIMEOUT, TimedOut),
            e(c::ERROR_RUNLEVEL_SWITCH_AGENT_TIMEOUT, TimedOut),
        ];
        let mut i = 1;
        while i < t.len() {
            assert!(t[i - 1].0 < t[i].0);
            i += 1;
        }
        t
    };

    // Split to avoid padding each entry to 4 bytes.
    static CODES: [u16; TABLE.len()] = {
        let mut codes = [0; TABLE.len()];
        let mut i = 0;
        while i < TABLE.len() {
            codes[i] = TABLE[i].0;
            i += 1;
        }
        codes
    };
    static KINDS: [io::ErrorKind; TABLE.len()] = {
        let mut kinds = [Uncategorized; TABLE.len()];
        let mut i = 0;
        while i < TABLE.len() {
            kinds[i] = TABLE[i].1;
            i += 1;
        }
        kinds
    };

    if errno as u32 > TABLE[TABLE.len() - 1].0 as u32 {
        return Uncategorized;
    }
    let code = errno as u16;
    match CODES.iter().position(|&c| c == code) {
        Some(i) => KINDS.get(i).copied().unwrap(),
        None => Uncategorized,
    }
}

/// Gets a detailed string description for the given error number.
pub fn format_error(mut errnum: i32, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut buf = [0 as c::WCHAR; 2048];

    unsafe {
        let mut module = ptr::null_mut();
        let mut flags = 0;

        // NTSTATUS errors may be encoded as HRESULT, which may returned from
        // GetLastError. For more information about Windows error codes, see
        // `[MS-ERREF]`: https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-erref/0642cb2f-2075-4469-918c-4441e69c548a
        if (errnum & c::FACILITY_NT_BIT as i32) != 0 {
            // format according to https://support.microsoft.com/en-us/help/259693
            module = c::GetModuleHandleA(c"NTDLL.DLL".as_ptr().cast());

            if !module.is_null() {
                errnum ^= c::FACILITY_NT_BIT as i32;
                flags = c::FORMAT_MESSAGE_FROM_HMODULE;
            }
        }

        let res = c::FormatMessageW(
            flags | c::FORMAT_MESSAGE_FROM_SYSTEM | c::FORMAT_MESSAGE_IGNORE_INSERTS,
            module,
            errnum as u32,
            0,
            buf.as_mut_ptr(),
            buf.len() as u32,
            ptr::null(),
        ) as usize;
        if res == 0 {
            // Sometimes FormatMessageW can fail e.g., system doesn't like 0 as langId,
            let fm_err = errno();
            return write!(f, "OS Error {errnum} (FormatMessageW() returned error {fm_err})");
        }

        match String::from_utf16(&buf[..res]) {
            Ok(msg) => {
                // Trim trailing CRLF inserted by FormatMessageW
                f.write_str(msg.trim_ascii_end())
            }
            Err(..) => write!(f, "OS Error {} (FormatMessageW() returned invalid UTF-16)", errnum),
        }
    }
}
