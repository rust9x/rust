use crate::sys::c;

/// Returns true if we are running on a Windows NT-based system. Only use this for APIs where the
/// same API differs in behavior or capability on 9x/ME compared to NT.
#[allow(dead_code)]
#[inline(always)]
#[cfg(target_arch = "x86")]
pub fn is_windows_nt() -> bool {
    unsafe { IS_NT }
}

#[allow(dead_code)]
#[inline(always)]
#[cfg(target_arch = "x86_64")]
pub fn is_windows_nt() -> bool {
    true // let me know once someone ported 9x to 64bit LOL
}

#[inline(always)]
pub fn supports_async_io() -> bool {
    unsafe { SUPPORTS_ASYNC_IO }
}

/// Whether the new way (just opening \??\PIPE\ / \Device\NamedPipe without a file/pipe name creates
/// an anon pipe) is supported.
///
/// Prior to Vista (NT 6), kernel32's `CreatePipe` would create a pipe with a unique name via
/// `_sprintf(Buffer, "\\Device\\NamedPipe\\Win32Pipes.%08x.%08x", process_id, global_counter)`;
///
/// see https://github.com/rust-lang/rust/pull/142517
#[inline(always)]
pub fn supports_anon_pipe_autoname() -> bool {
    unsafe { SUPPORTS_ANON_PIPE_AUTONAME }
}

pub fn init_rust9x_checks() {
    // DO NOT do anything interesting or complicated in this function! DO NOT call
    // any Rust functions or CRT functions if those functions touch any global state,
    // because this function runs during global initialization. For example, DO NOT
    // do any dynamic allocation, don't call LoadLibrary, etc.

    init_windows_version_check();
    init_mutex_kind_check();
}

static mut IS_NT: bool = false;
static mut SUPPORTS_ASYNC_IO: bool = false;
static mut SUPPORTS_ANON_PIPE_AUTONAME: bool = false;

fn init_windows_version_check() {
    // according to old MSDN info, the high-order bit is set only on 95/98/ME.
    unsafe {
        let version = c::GetVersion();
        IS_NT = version < 0x8000_0000;
        SUPPORTS_ASYNC_IO = IS_NT && c::CancelIo::available().is_some();
        SUPPORTS_ANON_PIPE_AUTONAME = IS_NT && version & 0xFF >= 0x06; // Vista+/NT6+
    };
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MutexKind {
    /// Win 7+ (Vista doesn't support the `Try*` APIs)
    SrwLock,
    /// NT 4+ (9x/ME/NT3.x support critical sections, but don't support `TryEnterCriticalSection`)
    CriticalSection,
    /// `CreateMutex`, available everywhere
    Legacy,
}

static mut MUTEX_KIND: MutexKind = MutexKind::Legacy;

#[inline(always)]
pub(crate) fn mutex_kind() -> MutexKind {
    unsafe { MUTEX_KIND }
}

fn init_mutex_kind_check() {
    let kind = if c::TryAcquireSRWLockExclusive::available().is_some() {
        MutexKind::SrwLock
    } else if {
        // Windows 9x exports `TryEnterCriticalSection`, but it returns ERROR_CALL_NOT_IMPLEMENTED.
        // MSDN specifies that the function is available on NT4 and later.
        is_windows_nt() && c::TryEnterCriticalSection::available().is_some()
    } {
        MutexKind::CriticalSection
    } else {
        MutexKind::Legacy
    };

    unsafe {
        MUTEX_KIND = kind;
    }
}
