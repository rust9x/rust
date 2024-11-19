//! A "compatibility layer" for supporting older versions of Windows
//!
//! The standard library uses some Windows API functions that are not present
//! on older versions of Windows.  (Note that the oldest version of Windows
//! that Rust supports is Windows 7 (client) and Windows Server 2008 (server).)
//! This module implements a form of delayed DLL import binding, using
//! `GetModuleHandle` and `GetProcAddress` to look up DLL entry points at
//! runtime.
//!
//! This is implemented simply by storing a function pointer in an atomic.
//! Loading and calling this function will have little or no overhead
//! compared with calling any other dynamically imported function.
//!
//! The stored function pointer starts out as an importer function which will
//! swap itself with the real function when it's called for the first time. If
//! the real function can't be imported then a fallback function is used in its
//! place. While this is low cost for the happy path (where the function is
//! already loaded) it does mean there's some overhead the first time the
//! function is called. In the worst case, multiple threads may all end up
//! importing the same function unnecessarily.

use crate::ffi::{CStr, c_void};
use crate::ptr::NonNull;
use crate::sys::c;

#[cfg(target_vendor = "rust9x")]
pub(crate) mod checks;

#[cfg(target_vendor = "rust9x")]
pub(crate) mod thread_parking;

// This uses a static initializer to preload some imported functions.
// The CRT (C runtime) executes static initializers before `main`
// is called (for binaries) and before `DllMain` is called (for DLLs).
//
// It works by contributing a global symbol to the `.CRT$XCT` section.
// The linker builds a table of all static initializer functions.
// The CRT startup code then iterates that table, calling each
// initializer function.
//
// NOTE: User code should instead use .CRT$XCU to reliably run after std's initializer.
// If you're reading this and would like a guarantee here, please
// file an issue for discussion; currently we don't guarantee any functionality
// before main.
// See https://docs.microsoft.com/en-us/cpp/c-runtime-library/crt-initialization?view=msvc-170
#[cfg(any(target_vendor = "win7", target_vendor = "rust9x"))]
#[used]
#[link_section = ".CRT$XCT"]
static INIT_TABLE_ENTRY: unsafe extern "C" fn() = init;

/// Preload some imported functions.
///
/// Note that any functions included here will be unconditionally loaded in
/// the final binary, regardless of whether or not they're actually used.
///
/// Therefore, this should be limited to `compat_fn_optional` functions which
/// must be preloaded or any functions where lazier loading demonstrates a
/// negative performance impact in practical situations.
///
/// Currently we only preload `WaitOnAddress` and `WakeByAddressSingle`.
#[cfg(target_vendor = "win7")]
unsafe extern "C" fn init() {
    // In an exe this code is executed before main() so is single threaded.
    // In a DLL the system's loader lock will be held thereby synchronizing
    // access. So the same best practices apply here as they do to running in DllMain:
    // https://docs.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-best-practices
    //
    // DO NOT do anything interesting or complicated in this function! DO NOT call
    // any Rust functions or CRT functions if those functions touch any global state,
    // because this function runs during global initialization. For example, DO NOT
    // do any dynamic allocation, don't call LoadLibrary, etc.

    // Attempt to preload the synch functions.
    load_synch_functions();
}

#[cfg(target_vendor = "rust9x")]
unsafe extern "C" fn init() {
    // In an exe this code is executed before main() so is single threaded.
    // In a DLL the system's loader lock will be held thereby synchronizing
    // access. So the same best practices apply here as they do to running in DllMain:
    // https://docs.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-best-practices
    //
    // DO NOT do anything interesting or complicated in this function! DO NOT call
    // any Rust functions or CRT functions if those functions touch any global state,
    // because this function runs during global initialization. For example, DO NOT
    // do any dynamic allocation, don't call LoadLibrary, etc.

    checks::init_rust9x_checks();
}

/// Helper macro for creating CStrs from literals and symbol names.
macro_rules! ansi_str {
    (sym $ident:ident) => {{ crate::sys::compat::const_cstr_from_bytes(concat!(stringify!($ident), "\0").as_bytes()) }};
    ($lit:literal) => {{ crate::sys::compat::const_cstr_from_bytes(concat!($lit, "\0").as_bytes()) }};
}

/// Creates a C string wrapper from a byte slice, in a constant context.
///
/// This is a utility function used by the [`ansi_str`] macro.
///
/// # Panics
///
/// Panics if the slice is not null terminated or contains nulls, except as the last item
pub(crate) const fn const_cstr_from_bytes(bytes: &'static [u8]) -> &'static CStr {
    if !matches!(bytes.last(), Some(&0)) {
        panic!("A CStr must be null terminated");
    }
    let mut i = 0;
    // At this point `len()` is at least 1.
    while i < bytes.len() - 1 {
        if bytes[i] == 0 {
            panic!("A CStr must not have interior nulls")
        }
        i += 1;
    }
    // SAFETY: The safety is ensured by the above checks.
    unsafe { crate::ffi::CStr::from_bytes_with_nul_unchecked(bytes) }
}

/// Represents a loaded module.
///
/// Note that the modules std depends on must not be unloaded.
/// Therefore a `Module` is always valid for the lifetime of std.
#[derive(Copy, Clone)]
pub(in crate::sys) struct Module(NonNull<c_void>);
impl Module {
    /// Try to get a handle to a loaded module.
    ///
    /// # SAFETY
    ///
    /// This should only be use for modules that exist for the lifetime of std
    /// (e.g. kernel32 and ntdll).
    pub unsafe fn new(name: &CStr) -> Option<Self> {
        // SAFETY: A CStr is always null terminated.
        unsafe {
            let module = c::GetModuleHandleA(name.as_ptr().cast::<u8>());
            NonNull::new(module).map(Self)
        }
    }

    #[allow(dead_code)]
    pub unsafe fn load(name: &CStr) -> Option<Self> {
        // SAFETY: A CStr is always null terminated.
        unsafe {
            let module = c::LoadLibraryA(name.as_ptr().cast::<u8>());
            NonNull::new(module).map(Self)
        }
    }

    // Try to get the address of a function.
    pub fn proc_address(self, name: &CStr) -> Option<NonNull<c_void>> {
        unsafe {
            // SAFETY:
            // `self.0` will always be a valid module.
            // A CStr is always null terminated.
            let proc = c::GetProcAddress(self.0.as_ptr(), name.as_ptr().cast::<u8>());
            // SAFETY: `GetProcAddress` returns None on null.
            proc.map(|p| NonNull::new_unchecked(p as *mut c_void))
        }
    }
}

pub static UNICOWS: &CStr = c"unicows";

/// Load a function or use a fallback implementation if that fails.
macro_rules! compat_fn_with_fallback {
    {
        pub static $module:ident: &CStr = $name:expr => { load: $load:expr, unicows: $unicows:expr };
        $(
            $(#[$meta:meta])*
            $vis:vis fn $symbol:ident($($argname:ident: $argtype:ty),* $(,)?) $(-> $rettype:ty)? $fallback_body:block
        )*
    } => {
    $(
        $(#[$meta])*
        pub mod $symbol {
            #[allow(unused_imports)]
            use super::*;
            use crate::mem;
            use crate::ffi::{CStr, c_void};
            use crate::sync::atomic::{AtomicPtr, Ordering};

            type F = unsafe extern "system" fn($($argtype),*) $(-> $rettype)?;

            /// `PTR` contains a function pointer to one of three functions.
            /// It starts with the `load` function.
            /// When that is called it attempts to load the requested symbol.
            /// If it succeeds, `PTR` is set to the address of that symbol.
            /// If it fails, then `PTR` is set to `fallback`.
            pub(in crate::sys) static PTR: AtomicPtr<c_void> = AtomicPtr::new(load as *mut _);

            unsafe extern "system" fn load($($argname: $argtype),*) $(-> $rettype)? {
                unsafe {
                    let func = load_from_module();
                    func($($argname),*)
                }
            }

            fn load_from_module() -> F {
                unsafe {
                    static SYMBOL_NAME: &CStr = ansi_str!(sym $symbol);

                    let f = crate::sys::compat::load_from_module(
                        $name,
                        SYMBOL_NAME,
                        $load,
                        $unicows
                    );

                    if let Some(f) = f {
                        PTR.store(f.as_ptr(), Ordering::Relaxed);
                        mem::transmute(f)
                    } else {
                        PTR.store(fallback as *mut _, Ordering::Relaxed);
                        fallback
                    }
                }
            }

            #[allow(dead_code)]
            pub fn available() -> Option<F> {
                let mut ptr = PTR.load(Ordering::Relaxed);
                if ptr == load as *mut _ {
                    ptr = load_from_module() as *mut _;
                }

                if ptr != fallback as *mut _ {
                    Some(unsafe { mem::transmute(ptr) })
                } else {
                    None
                }
            }

            #[allow(unused_variables)]
            unsafe extern "system" fn fallback($($argname: $argtype),*) $(-> $rettype)? {
                $fallback_body
            }

            #[inline(always)]
            pub unsafe fn call($($argname: $argtype),*) $(-> $rettype)? {
                unsafe {
                    let func: F = mem::transmute(PTR.load(Ordering::Relaxed));
                    func($($argname),*)
                }
            }
        }
        #[allow(unused)]
        $(#[$meta])*
        $vis use $symbol::call as $symbol;
    )*
    }
}

/// Optionally loaded functions.
///
/// Relies on the functions being pre-loaded elsewhere.
#[cfg(target_vendor = "win7")]
macro_rules! compat_fn_optional {
    ($(
        $(#[$meta:meta])*
        $vis:vis fn $symbol:ident($($argname:ident: $argtype:ty),* $(,)?) $(-> $rettype:ty)?;
    )+) => (
        $(
            pub mod $symbol {
                #[allow(unused_imports)]
                use super::*;
                use crate::ffi::c_void;
                use crate::mem;
                use crate::ptr::{self, NonNull};
                use crate::sync::atomic::{AtomicPtr, Ordering};

                pub(in crate::sys) static PTR: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

                type F = unsafe extern "system" fn($($argtype),*) $(-> $rettype)?;

                #[inline(always)]
                #[allow(dead_code)]
                pub fn option() -> Option<F> {
                    unsafe {
                        NonNull::new(PTR.load(Ordering::Relaxed)).map(|f| unsafe { mem::transmute(f) })
                    }
                }

                #[inline(always)]
                #[allow(dead_code)]
                pub unsafe fn call($($argname: $argtype),*) $(-> $rettype)? {
                    unsafe {
                        (mem::transmute::<_, F>(PTR.load(Ordering::Relaxed)))($($argname),*)
                    }
                }
            }
            #[inline]
            pub unsafe extern "system" fn $symbol($($argname: $argtype),*) $(-> $rettype)? {
                unsafe { $symbol::option().unwrap()($($argname),*) }
            }
        )+
    )
}

pub(crate) fn load_from_module(
    module_name: &CStr,
    symbol_name: &CStr,
    load: bool,
    unicows: bool,
) -> Option<NonNull<c_void>> {
    let in_unicows = if unicows {
        unsafe { Module::new(UNICOWS).and_then(|m| m.proc_address(symbol_name)) }
    } else {
        None
    };

    in_unicows.or_else(|| {
        if load {
            unsafe { Module::new(module_name) }
        } else {
            unsafe { Module::load(module_name) }
        }
        .and_then(|m| m.proc_address(symbol_name))
    })
}

/// Load all needed functions from "api-ms-win-core-synch-l1-2-0".
#[cfg(target_vendor = "win7")]
pub(super) fn load_synch_functions() {
    fn try_load() -> Option<()> {
        use crate::sync::atomic::Ordering;
        const MODULE_NAME: &CStr = c"api-ms-win-core-synch-l1-2-0";
        const WAIT_ON_ADDRESS: &CStr = c"WaitOnAddress";
        const WAKE_BY_ADDRESS_SINGLE: &CStr = c"WakeByAddressSingle";

        // Try loading the library and all the required functions.
        // If any step fails, then they all fail.
        let library = unsafe { Module::new(MODULE_NAME) }?;
        let wait_on_address = library.proc_address(WAIT_ON_ADDRESS)?;
        let wake_by_address_single = library.proc_address(WAKE_BY_ADDRESS_SINGLE)?;

        c::WaitOnAddress::PTR.store(wait_on_address.as_ptr(), Ordering::Relaxed);
        c::WakeByAddressSingle::PTR.store(wake_by_address_single.as_ptr(), Ordering::Relaxed);
        Some(())
    }
}
