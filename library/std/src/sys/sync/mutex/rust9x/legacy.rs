use crate::cell::UnsafeCell;
use crate::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use crate::sys::sync::OnceBox;
use crate::sys::{c, cvt};
use crate::{io, ptr};

/// Mutex based on `CreateMutex`. Slow, but available everywhere.
///
/// Doesn't need to stay fixed in place, but we need to lazily initialize it to have a const
/// constructor, so we use a `OnceBox` for that.
pub struct LegacyMutex {
    pub(crate) inner: OnceBox<OwnedHandle>,
    // used to prevent reentrancy:
    //
    // > The exact behavior on locking a mutex in the thread which already holds the lock is left
    // > unspecified. However, this function will not return on the second call (it might panic or
    // > deadlock, for example).
    held: UnsafeCell<bool>,
}

unsafe impl Send for LegacyMutex {}
unsafe impl Sync for LegacyMutex {}

impl LegacyMutex {
    #[inline]
    #[allow(dead_code, reason = "initialized via rust9x::Mutex::new")]
    pub const fn new() -> Self {
        Self { inner: OnceBox::new(), held: UnsafeCell::new(false) }
    }

    fn init() -> Box<OwnedHandle> {
        unsafe {
            let handle = c::CreateMutexA(ptr::null_mut(), c::FALSE, ptr::null());
            if handle.is_null() {
                panic!("failed creating mutex: {}", io::Error::last_os_error());
            }

            Box::new(OwnedHandle::from_raw_handle(handle))
        }
    }

    #[inline]
    pub unsafe fn lock(&self) {
        let handle = self.inner.get_or_init(Self::init);
        if c::WaitForSingleObject(handle.as_raw_handle(), c::INFINITE) != c::WAIT_OBJECT_0 {
            panic!("mutex lock failed: {}", io::Error::last_os_error())
        }

        if !self.flag_locked() {
            self.unlock();
            panic!("cannot recursively lock a mutex");
        }
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        let handle = self.inner.get_or_init(Self::init);
        let successful = match c::WaitForSingleObject(handle.as_raw_handle(), 0) {
            c::WAIT_OBJECT_0 => true,
            c::WAIT_TIMEOUT => false,
            _ => panic!("try lock error: {}", io::Error::last_os_error()),
        };

        if !successful {
            false
        } else if self.flag_locked() {
            true
        } else {
            self.unlock();
            false
        }
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        *self.held.get() = false;

        // SAFETY: The outer mutex code prevents calls to unlock before lock, so the mutex is
        // guaranteed to be initialized.
        let handle = self.inner.get_unchecked();
        cvt(c::ReleaseMutex(handle.as_raw_handle())).unwrap();
    }

    unsafe fn flag_locked(&self) -> bool {
        if *self.held.get() {
            false
        } else {
            *self.held.get() = true;
            true
        }
    }
}

impl Drop for LegacyMutex {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            if self.try_lock() {
                self.unlock();
            } else {
                // The mutex is locked. This happens if a MutexGuard is leaked.
                // In this case, we have to leak the Mutex too.
                core::mem::forget(self.inner.take());
            }
        }
    }
}
