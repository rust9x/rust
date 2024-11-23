use super::windows7;
use crate::mem::{ManuallyDrop, MaybeUninit};
use crate::sys::compat::checks::{MutexKind, mutex_kind};
use crate::sys::sync::Mutex;
use crate::time::Duration;

mod legacy;

pub union Condvar {
    windows7: ManuallyDrop<windows7::Condvar>,
    legacy: ManuallyDrop<legacy::Condvar>,
}

impl Drop for Condvar {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            match mutex_kind() {
                MutexKind::SrwLock => ManuallyDrop::drop(&mut self.windows7),
                MutexKind::CriticalSection | MutexKind::Legacy => {
                    ManuallyDrop::drop(&mut self.legacy)
                }
            }
        }
    }
}

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    #[inline]
    pub const fn new() -> Condvar {
        // SAFETY: all variants are valid when zero-initialized:
        // - Windows 7 CONDITION_VARIABLE: The initialized value `CONDITION_VARIABLE_INIT` is a zero
        //   pointer value.
        // - Legacy: The `OnceBox` is valid when zero-initialized.
        unsafe { MaybeUninit::zeroed().assume_init() }
    }

    #[inline]
    pub unsafe fn wait(&self, mutex: &Mutex) {
        match mutex_kind() {
            MutexKind::SrwLock => self.windows7.wait(mutex),
            MutexKind::CriticalSection | MutexKind::Legacy => self.legacy.wait(mutex),
        }
    }

    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        match mutex_kind() {
            MutexKind::SrwLock => self.windows7.wait_timeout(mutex, dur),
            MutexKind::CriticalSection | MutexKind::Legacy => self.legacy.wait_timeout(mutex, dur),
        }
    }

    #[inline]
    pub fn notify_one(&self) {
        unsafe {
            match mutex_kind() {
                MutexKind::SrwLock => self.windows7.notify_one(),
                MutexKind::CriticalSection | MutexKind::Legacy => self.legacy.notify_one(),
            }
        }
    }

    #[inline]
    pub fn notify_all(&self) {
        unsafe {
            match mutex_kind() {
                MutexKind::SrwLock => self.windows7.notify_all(),
                MutexKind::CriticalSection | MutexKind::Legacy => self.legacy.notify_all(),
            }
        }
    }
}
