static mut THREAD_PARKING_IMPL: Option<ThreadParkingImpl> = None;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThreadParkingImpl {
    // WaitOnAddress-based, Windows 8+
    Futex,
    // NtCreatedKeyedEvent-based, Windows XP+
    KeyedEvent,
    // Generic thread parker based on Mutex+Condvar
    Generic,
}

// This CANNOT be called during global init because it has to `LoadLibrary` to figure out which impl
// is available.
pub(crate) fn thread_parking_impl() -> ThreadParkingImpl {
    if let Some(implementation) = unsafe { THREAD_PARKING_IMPL } {
        return implementation;
    }

    let implementation = {
        if crate::sys::c::WaitOnAddress::available().is_some()
            && crate::sys::c::WakeByAddressSingle::available().is_some()
        {
            ThreadParkingImpl::Futex
        } else if crate::sys::c::NtCreateKeyedEvent::available().is_some()
            && crate::sys::c::NtReleaseKeyedEvent::available().is_some()
            && crate::sys::c::NtWaitForKeyedEvent::available().is_some()
        {
            ThreadParkingImpl::KeyedEvent
        } else {
            ThreadParkingImpl::Generic
        }
    };

    unsafe {
        THREAD_PARKING_IMPL = Some(implementation);
    }

    implementation
}
