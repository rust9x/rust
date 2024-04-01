use crate::sys::c;

static mut IS_NT: bool = true;
static mut SUPPORTS_ASYNC_IO: bool = true;
static mut SUPPORTS_TRY_ENTER_CRITICAL_SECTION: bool = true;

pub fn init_windows_version_check() {
    unsafe {
        let version = c::GetVersion();
        let major = version as u8;

        // according to old MSDN info, the high-order bit is set only on 95/98/ME.
        let is_nt = version < 0x8000_0000;

        IS_NT = is_nt;
        SUPPORTS_ASYNC_IO = is_nt && c::CancelIo::option().is_some();

        // at least 9x exports TryEnterCriticalSection, but it doesn't work, so we need to check the
        // version. MSDN specifies that the function is available on NT4 and later.
        SUPPORTS_TRY_ENTER_CRITICAL_SECTION = is_nt && major >= 4;
    };
}

/// Returns true if we are running on a Windows NT-based system. Only use this for APIs where the
/// same API differs in behavior or capability on 9x/ME compared to NT.
#[inline(always)]
pub fn is_windows_nt() -> bool {
    unsafe { IS_NT }
}

#[inline(always)]
pub fn supports_async_io() -> bool {
    unsafe { SUPPORTS_ASYNC_IO }
}

#[inline(always)]
pub fn supports_try_enter_critical_section() -> bool {
    unsafe { SUPPORTS_TRY_ENTER_CRITICAL_SECTION }
}
