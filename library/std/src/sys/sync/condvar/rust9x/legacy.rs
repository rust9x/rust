use crate::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use crate::sys::c;
use crate::sys::compat::checks::{MutexKind, mutex_kind};
use crate::sys::pal::{cvt, dur2timeout};
use crate::sys::sync::{Mutex, OnceBox};
use crate::time::Duration;
use crate::{io, ptr};

/// Very basic condvar implementation for pre-Win7.
pub struct Condvar {
    inner: OnceBox<OwnedHandle>,
}

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    #[inline]
    #[cfg_attr(
        target_vendor = "rust9x",
        allow(dead_code, reason = "initialized via rust9x::Mutex::new")
    )]
    pub const fn new() -> Condvar {
        Condvar { inner: OnceBox::new() }
    }

    fn init() -> Box<OwnedHandle> {
        unsafe {
            let event = c::CreateEventA(
                ptr::null_mut(),
                c::TRUE, // manual reset event
                c::FALSE,
                ptr::null(),
            );

            if event.is_null() {
                panic!("failed creating event: {}", io::Error::last_os_error());
            }

            Box::new(OwnedHandle::from_raw_handle(event))
        }
    }

    #[inline]
    pub unsafe fn wait(&self, mutex: &Mutex) {
        let event = self.inner.get_or_init(Self::init);
        let use_signal_object_and_wait = if mutex_kind() == MutexKind::Legacy {
            c::SignalObjectAndWait::available()
        } else {
            None
        };

        unsafe {
            if let Some(signal_object_and_wait) = use_signal_object_and_wait {
                if signal_object_and_wait(
                    mutex.legacy.inner.get_unchecked().as_raw_handle(),
                    event.as_raw_handle(),
                    c::INFINITE,
                    c::FALSE,
                ) != c::WAIT_OBJECT_0
                {
                    panic!("event wait failed: {}", io::Error::last_os_error())
                }
                mutex.lock();
            } else {
                mutex.unlock();
                if (c::WaitForSingleObject(event.as_raw_handle(), c::INFINITE)) != c::WAIT_OBJECT_0
                {
                    panic!("event wait failed: {}", io::Error::last_os_error())
                }
                mutex.lock();
            }
        }
    }

    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        let event = self.inner.get_or_init(Self::init);
        let use_signal_object_and_wait = if mutex_kind() == MutexKind::Legacy {
            c::SignalObjectAndWait::available()
        } else {
            None
        };

        unsafe {
            if let Some(signal_object_and_wait) = use_signal_object_and_wait {
                let ret = match signal_object_and_wait(
                    mutex.legacy.inner.get_unchecked().as_raw_handle(),
                    event.as_raw_handle(),
                    dur2timeout(dur),
                    c::FALSE,
                ) {
                    c::WAIT_OBJECT_0 => true,
                    c::WAIT_TIMEOUT => false,
                    _ => panic!("event wait failed: {}", io::Error::last_os_error()),
                };
                mutex.lock();

                ret
            } else {
                mutex.unlock();
                let ret = match c::WaitForSingleObject(event.as_raw_handle(), dur2timeout(dur)) {
                    c::WAIT_OBJECT_0 => true,
                    c::WAIT_TIMEOUT => false,
                    _ => panic!("event wait failed: {}", io::Error::last_os_error()),
                };
                mutex.lock();

                ret
            }
        }
    }

    #[inline]
    pub fn notify_one(&self) {
        // lots of spurious wakeups, but that's valid
        self.notify_all();
    }

    #[inline]
    pub fn notify_all(&self) {
        let event = self.inner.get_or_init(Self::init);

        unsafe {
            cvt(c::SetEvent(event.as_raw_handle())).unwrap();
            crate::thread::yield_now();
            cvt(c::ResetEvent(event.as_raw_handle())).unwrap();
        }
    }
}
