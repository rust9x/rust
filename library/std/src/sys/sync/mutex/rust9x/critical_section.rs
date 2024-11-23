use crate::cell::UnsafeCell;
use crate::mem::MaybeUninit;
use crate::pin::Pin;
use crate::sys::c;
#[cfg(target_arch = "x86")]
use crate::sys::compat::checks;
use crate::sys::sync::OnceBox;

/// Attempts to enter `cs` via the correct `TryEnterCriticalSection` implementation for the running
/// Windows version. NT 4+ ships a working one; on NT 3.x it is missing and on 9x/ME it is a
/// non-functional stub, so on x86 we go through a function pointer resolved at startup that points
/// at either the native export or one of our reimplementations (see `checks`).
#[inline]
unsafe fn try_enter_critical_section(cs: *mut c::CRITICAL_SECTION) -> bool {
    #[cfg(target_arch = "x86")]
    let result = unsafe { (checks::try_enter_critical_section_fn())(cs) };
    #[cfg(not(target_arch = "x86"))]
    let result = unsafe { c::TryEnterCriticalSection(cs) };

    result != 0
}

/// Mutex based on critical sections.
///
/// Critical sections are available on all windows versions, but `TryEnterCriticalSection` was only
/// added with NT4. It is missing on NT 3.x and a non-functional stub on the 9x range, where we
/// provide our own reimplementations (see [`try_enter_critical_section`]).
///
/// Critical sections cannot be moved while initialized, so they have to be boxed. For this reason
/// we use `OnceBox`, which also allows for a `const` constructor.
pub struct CriticalSectionMutex {
    inner: OnceBox<UnsafeCell<c::CRITICAL_SECTION>>,
    // used to prevent reentrancy:
    //
    // > The exact behavior on locking a mutex in the thread which already holds the lock is left
    // > unspecified. However, this function will not return on the second call (it might panic or
    // > deadlock, for example).
    held: UnsafeCell<bool>,
}

unsafe impl Send for CriticalSectionMutex {}
unsafe impl Sync for CriticalSectionMutex {}

impl CriticalSectionMutex {
    #[inline]
    #[allow(dead_code, reason = "initialized via rust9x::Mutex::new")]
    pub const fn new() -> Self {
        Self { inner: OnceBox::new(), held: UnsafeCell::new(false) }
    }

    fn init() -> Pin<Box<UnsafeCell<c::CRITICAL_SECTION>>> {
        unsafe {
            let boxed = Box::pin(UnsafeCell::new(MaybeUninit::zeroed().assume_init()));
            c::InitializeCriticalSection(UnsafeCell::get(&boxed));
            boxed
        }
    }

    #[inline]
    pub unsafe fn lock(&self) {
        let cell = self.inner.get_or_init(Self::init);
        c::EnterCriticalSection(UnsafeCell::get(&cell));

        if !self.flag_locked() {
            self.unlock();
            panic!("cannot recursively lock a mutex");
        }
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        let cell = self.inner.get_or_init(Self::init);
        let successful = try_enter_critical_section(UnsafeCell::get(&cell));

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
        let cell = self.inner.get_unchecked();
        c::LeaveCriticalSection(UnsafeCell::get(&cell));
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

impl Drop for CriticalSectionMutex {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            if self.try_lock() {
                self.unlock();
                let cell = self.inner.get_unchecked();
                c::DeleteCriticalSection(UnsafeCell::get(&cell));
            } else {
                // The mutex is locked. This happens if a MutexGuard is leaked.
                // In this case, we have to leak the Mutex too.
                core::mem::forget(self.inner.take());
            }
        }
    }
}
