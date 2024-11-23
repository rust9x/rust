//! A reimplementation of `TryEnterCriticalSection` for Windows NT 3.x.
//!
//! `TryEnterCriticalSection` was only added to `kernel32` in NT 4. The NT 3.x releases
//! (3.1 / 3.5 / 3.51) ship critical sections but no `Try*` variant, so on those we implement it
//! ourselves.

use core::arch::asm;

use crate::ffi::c_void;
use crate::sync::atomic::{AtomicI32, Ordering};
use crate::sys::c;

/// Reads `NtCurrentTeb()->ClientId.UniqueThread`, the current thread's unique id.
unsafe fn current_thread_id() -> *mut c_void {
    // `fs:[0x18]` holds the linear address of the TEB; `ClientId` sits at offset 0x20 and its
    // second member, `UniqueThread`, at 0x24. These offsets are the same on every NT version.
    let teb: *const u8;
    unsafe {
        asm!("mov {}, fs:[0x18]", out(reg) teb, options(nostack, readonly, preserves_flags));
    }
    unsafe { teb.add(0x24).cast::<*mut c_void>().read() }
}

/// Attempts to enter the critical section without blocking, returning whether it was acquired.
///
/// # Safety
///
/// `cs` must point at a valid, initialized critical section, and the process must be running on
/// Windows NT.
pub(crate) unsafe extern "system" fn try_enter(cs: *mut c::CRITICAL_SECTION) -> c::BOOL {
    // `LockCount` sits at -1 while the lock is free and counts upward as it is (recursively) taken.
    let lock_count = unsafe { AtomicI32::from_ptr(&raw mut (*cs).LockCount) };
    let current_thread_id = unsafe { current_thread_id() };

    if lock_count.compare_exchange(-1, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
        // The lock was free; we now own it.
        unsafe {
            (*cs).OwningThread = current_thread_id;
            (*cs).RecursionCount = 1;
        }
        c::TRUE
    } else if unsafe { (*cs).OwningThread } == current_thread_id {
        // We already hold the lock; take it recursively.
        lock_count.fetch_add(1, Ordering::Relaxed);
        unsafe {
            (*cs).RecursionCount += 1;
        }
        c::TRUE
    } else {
        // Held by another thread.
        c::FALSE
    }
}
