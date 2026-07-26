use crate::ffi::{OsStr, OsString};
use crate::os::windows::prelude::*;
use crate::sys::pal::{c, cvt, fill_utf16_buf, to_u16s};
use crate::{fmt, io, ptr, slice};

pub struct Env {
    base: *mut c::WCHAR,
    iter: EnvIterator,
}

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { base: _, iter } = self;
        f.debug_list().entries(iter.clone()).finish()
    }
}

impl Iterator for Env {
    type Item = (OsString, OsString);

    fn next(&mut self) -> Option<(OsString, OsString)> {
        let Self { base: _, iter } = self;
        iter.next()
    }
}

#[derive(Clone)]
struct EnvIterator(*mut c::WCHAR);

impl Iterator for EnvIterator {
    type Item = (OsString, OsString);

    fn next(&mut self) -> Option<(OsString, OsString)> {
        #[cfg(target_family = "rust9x")]
        if crate::sys::c::GetEnvironmentStringsW::available().is_none() {
            return Self::nt31_next(self);
        }

        let Self(cur) = self;
        loop {
            unsafe {
                if **cur == 0 {
                    return None;
                }
                let p = *cur as *const u16;
                let mut len = 0;
                while *p.add(len) != 0 {
                    len += 1;
                }
                let s = slice::from_raw_parts(p, len);
                *cur = cur.add(len + 1);

                // Windows allows environment variables to start with an equals
                // symbol (in any other position, this is the separator between
                // variable name and value). Since`s` has at least length 1 at
                // this point (because the empty string terminates the array of
                // environment variables), we can safely slice.
                let pos = match s[1..].iter().position(|&u| u == b'=' as u16).map(|p| p + 1) {
                    Some(p) => p,
                    None => continue,
                };
                return Some((
                    OsStringExt::from_wide(&s[..pos]),
                    OsStringExt::from_wide(&s[pos + 1..]),
                ));
            }
        }
    }
}

#[cfg(target_family = "rust9x")]
impl EnvIterator {
    /// On NT 3.1, the env strings are actually not unicode, but in the active code page.
    #[cold]
    fn nt31_next(&mut self) -> Option<(OsString, OsString)> {
        use crate::sys::c::MultiByteToWideChar;
        loop {
            unsafe {
                let cur: &mut *mut u8 = &mut *(&raw mut self.0).cast::<*mut u8>();

                if **cur == 0 {
                    return None;
                }
                let p = *cur as *const u8;
                let mut len = 0;
                while *p.add(len) != 0 {
                    len += 1;
                }
                let s = slice::from_raw_parts(p, len);
                *cur = cur.add(len + 1);

                let pos = match s[1..].iter().position(|&u| u == b'=').map(|p| p + 1) {
                    Some(p) => p,
                    None => continue,
                };

                // size limit as reverse-engineered from NT 3.1's cmd.exe
                let mut buf = [core::mem::MaybeUninit::<u16>::uninit(); 1024];

                let len = MultiByteToWideChar(
                    0,
                    0,
                    s.as_ptr().cast(),
                    len as _,
                    buf.as_mut_ptr().cast(),
                    1024,
                ) as usize;

                return Some((
                    OsStringExt::from_wide(buf[..pos].assume_init_ref()),
                    OsStringExt::from_wide(buf[pos + 1..len].assume_init_ref()),
                ));
            }
        }
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        unsafe {
            c::FreeEnvironmentStringsW(self.base);
        }
    }
}

pub fn env() -> Env {
    unsafe {
        let ch = c::GetEnvironmentStringsW();
        if ch.is_null() {
            panic!("failure getting env string from OS: {}", io::Error::last_os_error());
        }
        Env { base: ch, iter: EnvIterator(ch) }
    }
}

pub fn getenv(k: &OsStr) -> Option<OsString> {
    let k = to_u16s(k).ok()?;
    fill_utf16_buf(
        |buf, sz| unsafe { c::GetEnvironmentVariableW(k.as_ptr(), buf, sz) },
        OsStringExt::from_wide,
    )
    .ok()
}

pub unsafe fn setenv(k: &OsStr, v: &OsStr) -> io::Result<()> {
    // SAFETY: We ensure that k and v are null-terminated wide strings.
    unsafe {
        let k = to_u16s(k)?;
        let v = to_u16s(v)?;

        cvt(c::SetEnvironmentVariableW(k.as_ptr(), v.as_ptr())).map(drop)
    }
}

pub unsafe fn unsetenv(n: &OsStr) -> io::Result<()> {
    // SAFETY: We ensure that v is a null-terminated wide strings.
    unsafe {
        let v = to_u16s(n)?;
        cvt(c::SetEnvironmentVariableW(v.as_ptr(), ptr::null())).map(drop)
    }
}
