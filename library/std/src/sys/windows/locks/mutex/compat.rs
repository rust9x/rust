use crate::sys::{c, compat};

#[derive(Debug, PartialEq)]
pub enum MutexKind {
    /// Win 7+ (Vista doesn't support the `Try*` APIs)
    SrwLock,
    /// NT 4+ (9x/ME/NT3.x support critical sections, but don't support `TryEnterCriticalSection`)
    CriticalSection,
    /// Good ol' `CreateMutex`, available everywhere
    Legacy,
}

pub static mut MUTEX_KIND: MutexKind = MutexKind::SrwLock;

pub fn init() {
    let kind = if c::TryAcquireSRWLockExclusive::option().is_some() {
        MutexKind::SrwLock
    } else if compat::supports_try_enter_critical_section()
        && c::TryEnterCriticalSection::option().is_some()
    {
        MutexKind::CriticalSection
    } else {
        MutexKind::Legacy
    };

    unsafe {
        MUTEX_KIND = kind;
    }
}
