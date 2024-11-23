//! A reimplementation of `TryEnterCriticalSection` for Windows 9x / ME.
//!
//! Based on https://fanael.github.io/static/try-enter-critsec-9x.cc,
//! from https://fanael.github.io/stockfish-on-windows-98.html,
//! licensed under CC0.

use core::arch::asm;

use crate::ffi::c_void;
use crate::sync::atomic::{AtomicI32, Ordering};
use crate::sys::c;
use crate::sys::compat::checks;

/// The tag identifying a 9x-layout critical section, as set by `InitializeCriticalSection`.
const CRITICAL_SECTION_TYPE: u8 = 4;

#[repr(C)]
struct CriticalSection {
    ty: u8,
    imp_98me: *mut CriticalSectionImpl,
    _reserved0: u32,
    imp_95: *mut CriticalSectionImpl,
    _reserved2: u32,
    _reserved3: u32,
}
const _: () = assert!(crate::mem::size_of::<CriticalSection>() == 24);

#[repr(C)]
struct CriticalSectionImpl {
    ty: u8,
    recursion_count: i32,
    owner_thread: *mut c_void,
    _reserved: u32,
    /// Sits at 1 while unlocked and counts downward as the lock is (recursively) taken.
    lock_count: AtomicI32,
    _internal_pointers: [*mut c_void; 3],
}
const _: () = assert!(crate::mem::size_of::<CriticalSectionImpl>() == 32);

/// Reads the pointer to the current thread's TDBX out of the TIB.
unsafe fn current_tdbx(offset: usize) -> *mut c_void {
    // `fs:[0x18]` holds the linear address of the TIB; the TDBX pointer lives at a version-specific
    // offset within it.
    let tib: *const u8;
    unsafe {
        asm!("mov {}, fs:[0x18]", out(reg) tib, options(nostack, readonly, preserves_flags));
    }
    unsafe { tib.add(offset).cast::<*mut c_void>().read() }
}

/// Attempts to enter the critical section without blocking, returning whether it was acquired.
///
/// # Safety
///
/// `cs` must point at a valid, initialized critical section, and the process must be running on
/// Windows 9x / ME.
pub(crate) unsafe extern "system" fn try_enter(cs: *mut c::CRITICAL_SECTION) -> c::BOOL {
    let cs = cs.cast::<CriticalSection>();

    // Every critical section we hand out here is set up by `InitializeCriticalSection`, so the tag
    // is always present; a mismatch would mean corruption or a foreign structure.
    debug_assert_eq!(unsafe { (*cs).ty }, CRITICAL_SECTION_TYPE);

    let imp = unsafe {
        if checks::is_win95() {
            core::hint::cold_path();
            (*cs).imp_95
        } else {
            (*cs).imp_98me
        }
    };
    let lock_count = unsafe { &(*imp).lock_count };
    let current_tdbx = unsafe { current_tdbx(checks::win9x_tdbx_offset()) };

    if lock_count.compare_exchange(1, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
        // The lock was free; we now own it.
        unsafe {
            (*imp).owner_thread = current_tdbx;
            (*imp).recursion_count += 1;
        }
        c::TRUE
    } else if unsafe { (*imp).owner_thread } == current_tdbx {
        // We already hold the lock; take it recursively.
        lock_count.fetch_sub(1, Ordering::Relaxed);
        unsafe {
            (*imp).recursion_count += 1;
        }
        c::TRUE
    } else {
        // Held by another thread.
        c::FALSE
    }
}
