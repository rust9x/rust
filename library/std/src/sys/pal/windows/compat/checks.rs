use crate::sys::c;
#[cfg(target_arch = "x86")]
use crate::sys::compat::{try_critical_section_9x, try_critical_section_nt3};

pub fn init_rust9x_checks() {
    // DO NOT do anything interesting or complicated in this function! DO NOT call
    // any Rust functions or CRT functions if those functions touch any global state,
    // because this function runs during global initialization. For example, DO NOT
    // do any dynamic allocation, don't call LoadLibrary, etc.

    init_windows_version_check();
    init_mutex_kind_check();
}

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

static mut IS_NT: bool = false;
static mut SUPPORTS_ASYNC_IO: bool = false;
static mut SUPPORTS_ANON_PIPE_AUTONAME: bool = false;

/// On Windows 9x / ME, the byte offset within the TIB (at `fs:[0x18]`) of the pointer to the
/// current thread's TDBX (thread database extension). This differs between 95/98 and ME.
#[cfg(target_arch = "x86")]
static mut WIN9X_TDBX_OFFSET: u8 = 0;

#[cfg(target_arch = "x86")]
#[inline(always)]
pub(crate) fn win9x_tdbx_offset() -> usize {
    unsafe { WIN9X_TDBX_OFFSET as usize }
}

/// The `CriticalSection` pointer to the internals is at a different offset on 95 vs 98/ME.
#[cfg(target_arch = "x86")]
static mut IS_WIN95: bool = false;

#[cfg(target_arch = "x86")]
#[inline(always)]
pub(crate) fn is_win95() -> bool {
    unsafe { IS_WIN95 }
}

fn init_windows_version_check() {
    // according to old MSDN info, the high-order bit is set only on 95/98/ME.
    unsafe {
        let version = c::GetVersion();
        IS_NT = version < 0x8000_0000;

        let major = (version & 0xFF) as u8;
        let minor = ((version >> 8) & 0xFF) as u8;

        SUPPORTS_ASYNC_IO = IS_NT && c::CancelIo::available().is_some();
        SUPPORTS_ANON_PIPE_AUTONAME = IS_NT && major >= 0x06; // Vista+/NT6+

        #[cfg(target_arch = "x86")]
        if !IS_NT {
            // The TDBX pointer is at offset 0x80 on ME and 0x50 on 95/98.
            const ME_MINOR: u8 = 90; // 4.90
            WIN9X_TDBX_OFFSET = if minor == ME_MINOR { 0x80 } else { 0x50 };
            IS_WIN95 = minor == 0; // 95 is 4.00
        }
    };
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MutexKind {
    /// Vista+.
    ///
    /// SRW locks and their condition variables exist from Vista on. `TryAcquireSRWLockExclusive`
    /// was only added in Win 7, but we reimplement it inline (see `windows7::Mutex::try_lock`), so
    /// this covers Vista too.
    SrwLock,
    /// Critical sections, used on every other version.
    ///
    /// The official `TryEnterCriticalSection` is only available on NT 4+. On NT 3.x and 9x/Me we
    /// substitute our own reimplementations. See `try_critical_section_nt3.rs` /
    /// `try_critical_section_9x.rs`.
    CriticalSection,
}

static mut MUTEX_KIND: MutexKind = MutexKind::CriticalSection;

#[inline(always)]
pub(crate) fn mutex_kind() -> MutexKind {
    unsafe { MUTEX_KIND }
}

#[cfg(target_arch = "x86")]
type TryEnterCriticalSectionFn = unsafe extern "system" fn(*mut c::CRITICAL_SECTION) -> c::BOOL;

/// The `TryEnterCriticalSection` implementation for the running Windows version, resolved once at
/// startup (see `init_mutex_kind_check`). On NT 4+ this is the real kernel32 export; NT 3.x doesn't
/// export it at all, and 9x/Me export a stub that just returns `ERROR_CALL_NOT_IMPLEMENTED`, so on
/// those we substitute our own reimplementations. The initial value is a placeholder, overwritten
/// before any mutex is used.
#[cfg(target_arch = "x86")]
static mut TRY_ENTER_CRITICAL_SECTION: TryEnterCriticalSectionFn =
    try_critical_section_9x::try_enter;

#[cfg(target_arch = "x86")]
#[inline(always)]
pub(crate) fn try_enter_critical_section_fn() -> TryEnterCriticalSectionFn {
    unsafe { TRY_ENTER_CRITICAL_SECTION }
}

fn init_mutex_kind_check() {
    unsafe {
        if c::AcquireSRWLockExclusive::available().is_some() {
            MUTEX_KIND = MutexKind::SrwLock;
            return;
        }

        // Everything else uses a critical section, falling back to a reimplemented
        // `TryEnterCriticalSection` where the native one is missing (see `mutex_kind`).
        #[cfg(target_arch = "x86")]
        {
            TRY_ENTER_CRITICAL_SECTION = if !is_windows_nt() {
                // 9x/Me: kernel32 exports a non-functional stub.
                try_critical_section_9x::try_enter
            } else if let Some(native) = c::TryEnterCriticalSection::available() {
                // NT 4+: `TryEnterCriticalSection` is available here.
                native
            } else {
                // NT 3.x: the `Try*` variant doesn't exist yet.
                try_critical_section_nt3::try_enter
            };
        }
    }
}
