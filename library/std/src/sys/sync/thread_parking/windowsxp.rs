// Just the Windows XP+ implementation yoinked from the Windows 7 implementation, without the
// Win8+/futex implementation around it, see super::windows7 for more info.

use core::pin::Pin;
use core::ptr;
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use core::sync::atomic::{AtomicI8, AtomicPtr};
use core::time::Duration;

use crate::ffi::c_void;
use crate::sys::c;

pub struct Parker {
    state: AtomicI8,
}

const PARKED: i8 = -1;
const EMPTY: i8 = 0;
const NOTIFIED: i8 = 1;

impl Parker {
    pub unsafe fn new_in_place(parker: *mut Parker) {
        parker.write(Self { state: AtomicI8::new(EMPTY) });
    }

    pub unsafe fn park(self: Pin<&Self>) {
        // Change NOTIFIED=>EMPTY or EMPTY=>PARKED, and directly return in the
        // first case.
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }

        // Wait for unpark() to produce this event.
        c::NtWaitForKeyedEvent(keyed_event_handle(), self.ptr(), 0, ptr::null_mut());
        // Set the state back to EMPTY (from either PARKED or NOTIFIED).
        // Note that we don't just write EMPTY, but use swap() to also
        // include an acquire-ordered read to synchronize with unpark()'s
        // release-ordered write.
        self.state.swap(EMPTY, Acquire);
        return;
    }

    pub unsafe fn park_timeout(self: Pin<&Self>, timeout: Duration) {
        // Change NOTIFIED=>EMPTY or EMPTY=>PARKED, and directly return in the
        // first case.
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }

        // Need to wait for unpark() using NtWaitForKeyedEvent.
        let handle = keyed_event_handle();

        // NtWaitForKeyedEvent uses a unit of 100ns, and uses negative
        // values to indicate a relative time on the monotonic clock.
        // This is documented here for the underlying KeWaitForSingleObject function:
        // https://docs.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-kewaitforsingleobject
        let mut timeout = match i64::try_from((timeout.as_nanos() + 99) / 100) {
            Ok(t) => -t,
            Err(_) => i64::MIN,
        };

        // Wait for unpark() to produce this event.
        let unparked =
            c::NtWaitForKeyedEvent(handle, self.ptr(), 0, &mut timeout) == c::STATUS_SUCCESS;

        // Set the state back to EMPTY (from either PARKED or NOTIFIED).
        let prev_state = self.state.swap(EMPTY, Acquire);

        if !unparked && prev_state == NOTIFIED {
            // We were awoken by a timeout, not by unpark(), but the state
            // was set to NOTIFIED, which means we *just* missed an
            // unpark(), which is now blocked on us to wait for it.
            // Wait for it to consume the event and unblock that thread.
            c::NtWaitForKeyedEvent(handle, self.ptr(), 0, ptr::null_mut());
        }
    }

    pub unsafe fn unpark(self: Pin<&Self>) {
        // Change PARKED=>NOTIFIED, EMPTY=>NOTIFIED, or NOTIFIED=>NOTIFIED, and
        // wake the thread in the first case.
        //
        // Note that even NOTIFIED=>NOTIFIED results in a write. This is on
        // purpose, to make sure every unpark() has a release-acquire ordering
        // with park().
        if self.state.swap(NOTIFIED, Release) == PARKED {
            // If we run NtReleaseKeyedEvent before the waiting thread runs
            // NtWaitForKeyedEvent, this (shortly) blocks until we can wake it up.
            // If the waiting thread wakes up before we run NtReleaseKeyedEvent
            // (e.g. due to a timeout), this blocks until we do wake up a thread.
            // To prevent this thread from blocking indefinitely in that case,
            // park_impl() will, after seeing the state set to NOTIFIED after
            // waking up, call NtWaitForKeyedEvent again to unblock us.
            c::NtReleaseKeyedEvent(keyed_event_handle(), self.ptr(), 0, ptr::null_mut());
        }
    }

    fn ptr(&self) -> *const c_void {
        (&raw const self.state).cast::<c_void>()
    }
}

fn keyed_event_handle() -> c::HANDLE {
    const INVALID: c::HANDLE = ptr::without_provenance_mut(!0);
    static HANDLE: AtomicPtr<crate::ffi::c_void> = AtomicPtr::new(INVALID);
    match HANDLE.load(Relaxed) {
        INVALID => {
            let mut handle = c::INVALID_HANDLE_VALUE;
            unsafe {
                match c::NtCreateKeyedEvent(
                    &mut handle,
                    c::GENERIC_READ | c::GENERIC_WRITE,
                    ptr::null_mut(),
                    0,
                ) {
                    c::STATUS_SUCCESS => {}
                    r => panic!("Unable to create keyed event handle: error {r}"),
                }
            }
            match HANDLE.compare_exchange(INVALID, handle, Relaxed, Relaxed) {
                Ok(_) => handle,
                Err(h) => {
                    // Lost the race to another thread initializing HANDLE before we did.
                    // Closing our handle and using theirs instead.
                    unsafe {
                        c::CloseHandle(handle);
                    }
                    h
                }
            }
        }
        handle => handle,
    }
}
