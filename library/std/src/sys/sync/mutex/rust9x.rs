use super::windows7;
use crate::mem::{ManuallyDrop, MaybeUninit};
use crate::sys::c;
use crate::sys::compat::checks::{MutexKind, mutex_kind};

mod critical_section;
mod legacy;

/// Only to be used with MutexKind::SrwLock.
#[inline]
pub unsafe fn raw(m: &Mutex) -> *mut c::SRWLOCK {
    unsafe { windows7::raw(&m.srwlock) }
}

pub union Mutex {
    pub(crate) srwlock: ManuallyDrop<windows7::Mutex>,
    critical_section: ManuallyDrop<critical_section::CriticalSectionMutex>,
    pub(crate) legacy: ManuallyDrop<legacy::LegacyMutex>,
}

unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

impl Drop for Mutex {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            match mutex_kind() {
                MutexKind::SrwLock => ManuallyDrop::drop(&mut self.srwlock),
                MutexKind::CriticalSection => ManuallyDrop::drop(&mut self.critical_section),
                MutexKind::Legacy => ManuallyDrop::drop(&mut self.legacy),
            }
        }
    }
}

impl Mutex {
    #[inline]
    pub const fn new() -> Mutex {
        // SAFETY: all variants are valid when zero-initialized:
        // - Windows 7 SRWLOCK: The initialized value `SRWLOCK_INIT` is a zero pointer value.
        // - Critical section: The `OnceBox` is valid when zero-initialized. `held` is a `bool` that
        //   should be initialized to `false`.
        // - Legacy: The `OnceBox` is valid when zero-initialized. `held` is a `bool` that should be
        //   initialized to `false`.
        unsafe { MaybeUninit::zeroed().assume_init() }
    }

    #[inline]
    pub fn lock(&self) {
        unsafe {
            match mutex_kind() {
                MutexKind::SrwLock => self.srwlock.lock(),
                MutexKind::CriticalSection => self.critical_section.lock(),
                MutexKind::Legacy => self.legacy.lock(),
            }
        }
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        unsafe {
            match mutex_kind() {
                MutexKind::SrwLock => self.srwlock.try_lock(),
                MutexKind::CriticalSection => self.critical_section.try_lock(),
                MutexKind::Legacy => self.legacy.try_lock(),
            }
        }
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        match mutex_kind() {
            MutexKind::SrwLock => self.srwlock.unlock(),
            MutexKind::CriticalSection => self.critical_section.unlock(),
            MutexKind::Legacy => self.legacy.unlock(),
        }
    }
}
