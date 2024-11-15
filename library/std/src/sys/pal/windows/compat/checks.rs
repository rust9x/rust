use crate::sys::c;

pub fn init_rust9x_checks() {
    // DO NOT do anything interesting or complicated in this function! DO NOT call
    // any Rust functions or CRT functions if those functions touch any global state,
    // because this function runs during global initialization. For example, DO NOT
    // do any dynamic allocation, don't call LoadLibrary, etc.

    init_windows_version_check();
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

static mut IS_NT: bool = true;

fn init_windows_version_check() {
    unsafe {
        let version = c::GetVersion();
        // according to old MSDN info, the high-order bit is set only on 95/98/ME.
        IS_NT = version < 0x8000_0000;

        let major = (version & 0xFF) as u8;
        let minor = ((version >> 8) & 0xFF) as u8;
    };
}
