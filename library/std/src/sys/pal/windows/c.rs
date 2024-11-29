//! C definitions used by libnative that don't belong in liblibc

#![allow(nonstandard_style)]
#![cfg_attr(test, allow(dead_code))]
#![unstable(issue = "none", feature = "windows_c")]
#![allow(clippy::style)]

#[allow(unused_imports)]
use core::ffi::c_void;
use core::ffi::{c_uint, c_ulong, c_ushort};
use core::{mem, ptr};

mod windows_sys;
pub use windows_sys::*;

#[cfg(target_vendor = "rust9x")]
pub(crate) mod wspiapi;

pub type WCHAR = u16;

pub const INVALID_HANDLE_VALUE: HANDLE = ::core::ptr::without_provenance_mut(-1i32 as _);

// https://learn.microsoft.com/en-us/cpp/c-runtime-library/exit-success-exit-failure?view=msvc-170
pub const EXIT_SUCCESS: u32 = 0;
pub const EXIT_FAILURE: u32 = 1;

#[cfg(any(target_vendor = "win7", target_vendor = "rust9x"))]
pub const CONDITION_VARIABLE_INIT: CONDITION_VARIABLE = CONDITION_VARIABLE { Ptr: ptr::null_mut() };
#[cfg(any(target_vendor = "win7", target_vendor = "rust9x"))]
pub const SRWLOCK_INIT: SRWLOCK = SRWLOCK { Ptr: ptr::null_mut() };
#[cfg(not(target_thread_local))]
pub const INIT_ONCE_STATIC_INIT: INIT_ONCE = INIT_ONCE { Ptr: ptr::null_mut() };

// Some windows_sys types have different signs than the types we use.
pub const OBJ_DONT_REPARSE: u32 = windows_sys::OBJ_DONT_REPARSE as u32;
pub const FRS_ERR_SYSVOL_POPULATE_TIMEOUT: u32 =
    windows_sys::FRS_ERR_SYSVOL_POPULATE_TIMEOUT as u32;

// Equivalent to the `NT_SUCCESS` C preprocessor macro.
// See: https://docs.microsoft.com/en-us/windows-hardware/drivers/kernel/using-ntstatus-values
pub fn nt_success(status: NTSTATUS) -> bool {
    status >= 0
}

impl UNICODE_STRING {
    pub fn from_ref(slice: &[u16]) -> Self {
        let len = mem::size_of_val(slice);
        Self { Length: len as _, MaximumLength: len as _, Buffer: slice.as_ptr() as _ }
    }
}

impl Default for OBJECT_ATTRIBUTES {
    fn default() -> Self {
        Self {
            Length: mem::size_of::<Self>() as _,
            RootDirectory: ptr::null_mut(),
            ObjectName: ptr::null_mut(),
            Attributes: 0,
            SecurityDescriptor: ptr::null_mut(),
            SecurityQualityOfService: ptr::null_mut(),
        }
    }
}

impl IO_STATUS_BLOCK {
    pub const PENDING: Self =
        IO_STATUS_BLOCK { Anonymous: IO_STATUS_BLOCK_0 { Status: STATUS_PENDING }, Information: 0 };
    pub fn status(&self) -> NTSTATUS {
        // SAFETY: If `self.Anonymous.Status` was set then this is obviously safe.
        // If `self.Anonymous.Pointer` was set then this is the equivalent to converting
        // the pointer to an integer, which is also safe.
        // Currently the only safe way to construct `IO_STATUS_BLOCK` outside of
        // this module is to call the `default` method, which sets the `Status`.
        unsafe { self.Anonymous.Status }
    }
}

/// NB: Use carefully! In general using this as a reference is likely to get the
/// provenance wrong for the `rest` field!
#[repr(C)]
pub struct REPARSE_DATA_BUFFER {
    pub ReparseTag: c_uint,
    pub ReparseDataLength: c_ushort,
    pub Reserved: c_ushort,
    pub rest: (),
}

/// NB: Use carefully! In general using this as a reference is likely to get the
/// provenance wrong for the `PathBuffer` field!
#[repr(C)]
pub struct SYMBOLIC_LINK_REPARSE_BUFFER {
    pub SubstituteNameOffset: c_ushort,
    pub SubstituteNameLength: c_ushort,
    pub PrintNameOffset: c_ushort,
    pub PrintNameLength: c_ushort,
    pub Flags: c_ulong,
    pub PathBuffer: WCHAR,
}

#[repr(C)]
pub struct MOUNT_POINT_REPARSE_BUFFER {
    pub SubstituteNameOffset: c_ushort,
    pub SubstituteNameLength: c_ushort,
    pub PrintNameOffset: c_ushort,
    pub PrintNameLength: c_ushort,
    pub PathBuffer: WCHAR,
}

// Desktop specific functions & types
cfg_if::cfg_if! {
if #[cfg(not(target_vendor = "uwp"))] {
    pub const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
}
}

// Use raw-dylib to import ProcessPrng as we can't rely on there being an import library.
#[cfg(not(any(target_vendor = "win7", target_vendor = "rust9x")))]
#[cfg_attr(
    target_arch = "x86",
    link(name = "bcryptprimitives", kind = "raw-dylib", import_name_type = "undecorated")
)]
#[cfg_attr(not(target_arch = "x86"), link(name = "bcryptprimitives", kind = "raw-dylib"))]
extern "system" {
    pub fn ProcessPrng(pbdata: *mut u8, cbdata: usize) -> BOOL;
}

// Functions that aren't available on every version of Windows that we support,
// but we still use them and just provide some form of a fallback implementation.
compat_fn_with_fallback! {
    pub static KERNEL32: &CStr = c"kernel32" => { load: false, unicows: false };

    // >= Win10 1607
    // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setthreaddescription
    pub fn SetThreadDescription(hthread: HANDLE, lpthreaddescription: PCWSTR) -> HRESULT {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); E_NOTIMPL }
    }

    // >= Win10 1607
    // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getthreaddescription
    pub fn GetThreadDescription(hthread: HANDLE, lpthreaddescription: *mut PWSTR) -> HRESULT {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); E_NOTIMPL }
    }

    // >= Win8 / Server 2012
    // https://docs.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimepreciseasfiletime
    #[cfg(any(target_vendor = "win7", target_vendor = "rust9x"))]
    pub fn GetSystemTimePreciseAsFileTime(lpsystemtimeasfiletime: *mut FILETIME) -> () {
        unsafe { GetSystemTimeAsFileTime(lpsystemtimeasfiletime) }
    }

    // >= Win11 / Server 2022
    // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-gettemppath2a
    pub fn GetTempPath2W(bufferlength: u32, buffer: PWSTR) -> u32 {
        unsafe {  GetTempPathW(bufferlength, buffer) }
    }
}

#[cfg(not(any(target_vendor = "win7", target_vendor = "rust9x")))]
// Use raw-dylib to import synchronization functions to workaround issues with the older mingw import library.
#[cfg_attr(
    target_arch = "x86",
    link(
        name = "api-ms-win-core-synch-l1-2-0",
        kind = "raw-dylib",
        import_name_type = "undecorated"
    )
)]
#[cfg_attr(
    not(target_arch = "x86"),
    link(name = "api-ms-win-core-synch-l1-2-0", kind = "raw-dylib")
)]
extern "system" {
    pub fn WaitOnAddress(
        address: *const c_void,
        compareaddress: *const c_void,
        addresssize: usize,
        dwmilliseconds: u32,
    ) -> BOOL;
    pub fn WakeByAddressSingle(address: *const c_void);
    pub fn WakeByAddressAll(address: *const c_void);
}

// These are loaded by `load_synch_functions`.
#[cfg(target_vendor = "win7")]
compat_fn_optional! {
    pub fn WaitOnAddress(
        address: *const c_void,
        compareaddress: *const c_void,
        addresssize: usize,
        dwmilliseconds: u32
    ) -> BOOL;
    pub fn WakeByAddressSingle(address: *const c_void);
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static SYNCH: &CStr = c"api-ms-win-core-synch-l1-2-0" => { load: true, unicows: false };

    pub fn WaitOnAddress(
        address: *const c_void,
        compareaddress: *const c_void,
        addresssize: usize,
        dwmilliseconds: u32
    ) -> BOOL {
        unimplemented!()
    }
    pub fn WakeByAddressSingle(address: *const c_void) {
        unimplemented!()
    }
    pub fn WakeByAddressAll(address: *const c_void) {
        unimplemented!()
    }
}

#[cfg(any(target_vendor = "win7", target_vendor = "rust9x", target_vendor = "uwp"))]
compat_fn_with_fallback! {
    pub static NTDLL: &CStr = c"ntdll" => { load: true, unicows: false };

    #[cfg(any(target_vendor = "win7", target_vendor = "rust9x"))]
    pub fn NtCreateKeyedEvent(
        KeyedEventHandle: *mut HANDLE,
        DesiredAccess: u32,
        ObjectAttributes: *mut c_void,
        Flags: u32
    ) -> NTSTATUS {
        panic!("keyed events not available")
    }
    #[cfg(any(target_vendor = "win7", target_vendor = "rust9x"))]
    pub fn NtReleaseKeyedEvent(
        EventHandle: HANDLE,
        Key: *const c_void,
        Alertable: BOOLEAN,
        Timeout: *mut i64
    ) -> NTSTATUS {
        panic!("keyed events not available")
    }
    #[cfg(any(target_vendor = "win7", target_vendor = "rust9x"))]
    pub fn NtWaitForKeyedEvent(
        EventHandle: HANDLE,
        Key: *const c_void,
        Alertable: BOOLEAN,
        Timeout: *mut i64
    ) -> NTSTATUS {
        panic!("keyed events not available")
    }

    // These functions are available on UWP when lazily loaded. They will fail WACK if loaded statically.
    #[cfg(target_vendor = "uwp")]
    pub fn NtCreateFile(
        filehandle: *mut HANDLE,
        desiredaccess: FILE_ACCESS_RIGHTS,
        objectattributes: *const OBJECT_ATTRIBUTES,
        iostatusblock: *mut IO_STATUS_BLOCK,
        allocationsize: *const i64,
        fileattributes: FILE_FLAGS_AND_ATTRIBUTES,
        shareaccess: FILE_SHARE_MODE,
        createdisposition: NTCREATEFILE_CREATE_DISPOSITION,
        createoptions: NTCREATEFILE_CREATE_OPTIONS,
        eabuffer: *const c_void,
        ealength: u32
    ) -> NTSTATUS {
        STATUS_NOT_IMPLEMENTED
    }
    #[cfg(target_vendor = "uwp")]
    pub fn NtReadFile(
        filehandle: HANDLE,
        event: HANDLE,
        apcroutine: PIO_APC_ROUTINE,
        apccontext: *const c_void,
        iostatusblock: *mut IO_STATUS_BLOCK,
        buffer: *mut c_void,
        length: u32,
        byteoffset: *const i64,
        key: *const u32
    ) -> NTSTATUS {
        STATUS_NOT_IMPLEMENTED
    }
    #[cfg(target_vendor = "uwp")]
    pub fn NtWriteFile(
        filehandle: HANDLE,
        event: HANDLE,
        apcroutine: PIO_APC_ROUTINE,
        apccontext: *const c_void,
        iostatusblock: *mut IO_STATUS_BLOCK,
        buffer: *const c_void,
        length: u32,
        byteoffset: *const i64,
        key: *const u32
    ) -> NTSTATUS {
        STATUS_NOT_IMPLEMENTED
    }
    #[cfg(target_vendor = "uwp")]
    pub fn RtlNtStatusToDosError(Status: NTSTATUS) -> u32 {
        Status as u32
    }
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static KERNEL32: &CStr = c"kernel32" => { load: false, unicows: false };

    // >= NT 4
    // https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-signalobjectandwait
    pub fn SignalObjectAndWait(
        hobjecttosignal: HANDLE,
        hobjecttowaiton: HANDLE,
        dwmilliseconds: u32,
        balertable: BOOL
    ) -> WAIT_EVENT { unimplemented!() }
    // >= NT 4
    // https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-tryentercriticalsection
    pub fn TryEnterCriticalSection(lpcriticalsection: *mut CRITICAL_SECTION) -> BOOL {
        unimplemented!()
    }
    // >= Win7 / Server 2008 R2
    // https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-tryacquiresrwlockexclusive
    pub fn TryAcquireSRWLockExclusive(srwlock: *mut SRWLOCK) -> BOOLEAN { unimplemented!() }
    pub fn TryAcquireSRWLockShared(srwlock: *mut SRWLOCK) -> BOOLEAN { unimplemented!() }
    // >= Vista / Server 2008
    // https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-acquiresrwlockexclusive
    pub fn AcquireSRWLockExclusive(srwlock: *mut SRWLOCK) -> () { unimplemented!() }
    pub fn AcquireSRWLockShared(srwlock: *mut SRWLOCK) -> () { unimplemented!() }
    pub fn ReleaseSRWLockExclusive(srwlock: *mut SRWLOCK) -> () { unimplemented!() }
    pub fn ReleaseSRWLockShared(srwlock: *mut SRWLOCK) -> () { unimplemented!() }
    pub fn SleepConditionVariableSRW(
        conditionvariable: *mut CONDITION_VARIABLE,
        srwlock: *mut SRWLOCK,
        dwmilliseconds: u32,
        flags: u32,
    ) -> BOOL { unimplemented!() }
    pub fn WakeAllConditionVariable(conditionvariable: *mut CONDITION_VARIABLE) -> () {
        unimplemented!()
    }
    pub fn WakeConditionVariable(conditionvariable: *mut CONDITION_VARIABLE) -> () {
        unimplemented!()
    }
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static KERNEL32: &CStr = c"kernel32" => { load: false, unicows: false };
    // >= XP
    // https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-addvectoredexceptionhandler
    pub fn AddVectoredExceptionHandler(
        first: u32,
        handler: PVECTORED_EXCEPTION_HANDLER
    ) -> *mut core::ffi::c_void { core::ptr::null_mut() }
    // >= Vista / Server 2003 SP1 / XPx64
    // https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setthreadstackguarantee
    pub fn SetThreadStackGuarantee(stacksizeinbytes: *mut u32) -> BOOL { TRUE }
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static KERNEL32: &CStr = c"kernel32" => { load: false, unicows: false };
    // >= 95 / NT 3.5
    // https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimeasfiletime
    pub fn GetSystemTimeAsFileTime(lpSystemTimeAsFileTime: *mut FILETIME) {
        unsafe {
            // implementation based on old MSDN docs
            let mut st: SYSTEMTIME = crate::mem::zeroed();
            GetSystemTime(&mut st);
            crate::sys::cvt(SystemTimeToFileTime(&st, lpSystemTimeAsFileTime)).unwrap();
        }
    }
    // >= NT 4
    // https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-switchtothread
    pub fn SwitchToThread() -> BOOL {
        unsafe { Sleep(0); }
        TRUE
    }

    // >= Vista / Server 2008
    // https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createwaitabletimerexw
    pub fn CreateWaitableTimerExW(
        lptimerattributes: *const SECURITY_ATTRIBUTES,
        lptimername: PCWSTR,
        dwflags: u32,
        dwdesiredaccess: u32
    ) -> HANDLE {
        ptr::null_mut()
    }

    // >= 98 / NT 4
    // https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-setwaitabletimer
    pub fn SetWaitableTimer(htimer: HANDLE,
        lpduetime: *const i64,
        lperiod: i32,
        pfncompletionroutine: PTIMERAPCROUTINE,
        lpargtocompletionroutine: *const core::ffi::c_void,
        fresume: BOOL
    ) -> BOOL {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); };
        FALSE
    }
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static KERNEL32: &CStr = c"kernel32" => { load: false, unicows: false };
    // >= 2000
    // https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfilepointerex
    pub fn SetFilePointerEx(
        hfile: HANDLE,
        lidistancetomove: i64,
        lpnewfilepointer: *mut i64,
        dwmovemethod: SET_FILE_POINTER_MOVE_METHOD,
    ) -> BOOL {
        unsafe {
            let distance_low = lidistancetomove as i32;
            let mut distance_high = (lidistancetomove >> 32) as i32;

            let new_pos_low = SetFilePointer(hfile, distance_low, &mut distance_high, dwmovemethod);

            // since (-1 as u32) could be a valid value for the lower 32 bits of the new file
            // pointer position, a call to GetLastError is needed to actually see if it failed
            if new_pos_low == INVALID_SET_FILE_POINTER && GetLastError() != NO_ERROR {
                return FALSE;
            }

            if !lpnewfilepointer.is_null() {
                *lpnewfilepointer = (distance_high as i64) << 32 | (new_pos_low as i64);
            }

            TRUE
        }
    }

    // >= Vista / Server 2008 (XP / Server 2003 when linking a supported FileExtd.lib)
    // https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle
    pub fn SetFileInformationByHandle(
        hfile: HANDLE,
        fileinformationclass: FILE_INFO_BY_HANDLE_CLASS,
        lpfileinformation: *const ::core::ffi::c_void,
        dwbuffersize: u32,
    ) -> BOOL {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); };
        FALSE
    }
    // >= Vista / Server 2008 (XP / Server 2003 when linking a supported FileExtd.lib)
    // https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
    pub fn GetFileInformationByHandleEx(
        hfile: HANDLE,
        fileinformationclass: FILE_INFO_BY_HANDLE_CLASS,
        lpfileinformation: *mut ::core::ffi::c_void,
        dwbuffersize: u32,
    ) -> BOOL {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); };
        FALSE
    }
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static KERNEL32: &CStr = c"kernel32" => { load: false, unicows: false };
    // >= Vista / Server 2008
    // https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createsymboliclinkw
    pub fn CreateSymbolicLinkW(
        lpsymlinkfilename: PCWSTR,
        lptargetfilename: PCWSTR,
        dwflags: SYMBOLIC_LINK_FLAGS,
    ) -> BOOLEAN {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); };
        0
    }
    // >= Vista / Server 2008
    // https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew
    pub fn GetFinalPathNameByHandleW(
        hfile: HANDLE,
        lpszfilepath: PWSTR,
        cchfilepath: u32,
        dwflags: GETFINALPATHNAMEBYHANDLE_FLAGS
    ) -> u32 {
        unimplemented!()
    }
    // >= 2000
    // https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createhardlinkw
    pub fn CreateHardLinkW(
        lpfilename: PCWSTR,
        lpexistingfilename: PCWSTR,
        lpsecurityattributes: *const SECURITY_ATTRIBUTES,
    ) -> BOOL {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); };
        FALSE
    }
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static advapi32: &CStr = c"advapi32" => { load: true, unicows: false };
    // >= XP / Server 2003
    // https://learn.microsoft.com/en-us/windows/win32/api/ntsecapi/nf-ntsecapi-rtlgenrandom
    pub fn SystemFunction036(
        randombuffer: *mut core::ffi::c_void,
        randombufferlength: u32
    ) -> BOOLEAN {
        unimplemented!()
    }

    // >= NT 4.0 / Windows 95 OSR2 / Windows 95 with IE 3.02
    // https://learn.microsoft.com/en-us/windows/win32/api/wincrypt/nf-wincrypt-cryptacquirecontexta
    pub fn CryptAcquireContextA(
        phprov: *mut usize,
        szcontainer: PCSTR,
        szprovider: PCSTR,
        dwprovtype: u32,
        dwflags: u32
    ) -> BOOL {
        unimplemented!()
    }
    pub fn CryptReleaseContext(hprov: usize, dwflags: u32) -> BOOL {
        unimplemented!()
    }
    pub fn CryptGenRandom(hprov: usize, dwlen: u32, pbbuffer: *mut u8) -> BOOL {
        unimplemented!()
    }
}
#[cfg(target_vendor = "rust9x")]
pub use self::SystemFunction036 as RtlGenRandom;

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static userenv: &CStr = c"userenv" => { load: true, unicows: false };
    // >= NT 4.0
    // https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getuserprofiledirectoryw
    pub fn GetUserProfileDirectoryW(
        htoken: HANDLE,
        lpprofiledir: PWSTR,
        lpcchsize: *mut u32
    ) -> BOOL {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); };
        FALSE
    }
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static advapi32: &CStr = c"advapi32" => { load: true, unicows: false };
    // >= NT
    // https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocesstoken
    pub fn OpenProcessToken(
        processhandle: HANDLE,
        desiredaccess: TOKEN_ACCESS_MASK,
        tokenhandle: *mut HANDLE
    ) -> BOOL {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); };
        FALSE
    }
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static KERNEL32: &CStr = c"kernel32" => { load: false, unicows: false };
    // >= NT 3.51+
    // https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-sethandleinformation
    pub fn SetHandleInformation(hobject: HANDLE, dwmask: u32, dwflags: HANDLE_FLAGS) -> BOOL {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); };
        FALSE
    }
}

#[cfg(target_vendor = "rust9x")]
mod ws2_32 {
    use super::*;
    compat_fn_with_fallback! {
        pub static WS2_32: &CStr = c"ws2_32" => { load: true, unicows: false };

        // >= NT4/2000 with IPv6 Tech Preview
        // https://learn.microsoft.com/en-us/windows/win32/api/ws2tcpip/nf-ws2tcpip-getaddrinfo
        pub fn getaddrinfo(
            pnodename: PCSTR,
            pservicename: PCSTR,
            phints: *const ADDRINFOA,
            ppresult: *mut *mut ADDRINFOA,
        ) -> i32 {
            unsafe { wship6::getaddrinfo(pnodename, pservicename, phints, ppresult) }
        }
        // >= NT4/2000 with IPv6 Tech Preview
        pub fn freeaddrinfo(paddrinfo: *const ADDRINFOA) -> () {
            unsafe { wship6::freeaddrinfo(paddrinfo) }
        }
    }
}
#[cfg(target_vendor = "rust9x")]
pub use ws2_32::{freeaddrinfo, getaddrinfo};

#[cfg(target_vendor = "rust9x")]
mod wship6 {
    use super::wspiapi::{wspiapi_freeaddrinfo, wspiapi_getaddrinfo};
    use super::{ADDRINFOA, PCSTR};

    compat_fn_with_fallback! {
        pub static WSHIP6: &CStr = c"wship6" => { load: true, unicows: false };

        // >= 2000 with IPv6 Tech Preview
        pub fn getaddrinfo(
            pnodename: PCSTR,
            pservicename: PCSTR,
            phints: *const ADDRINFOA,
            ppresult: *mut *mut ADDRINFOA,
        ) -> i32 {
            unsafe { wspiapi_getaddrinfo(pnodename, pservicename, phints, ppresult) }
        }
        // >= 2000 with IPv6 Tech Preview
        pub fn freeaddrinfo(paddrinfo: *const ADDRINFOA)-> () {
            unsafe { wspiapi_freeaddrinfo(paddrinfo) }
        }
    }
}

#[cfg(target_vendor = "rust9x")]
compat_fn_with_fallback! {
    pub static KERNEL32: &CStr = c"kernel32" => { load: false, unicows: false };
    // >= Vista / Server 2008
    // https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-initializeprocthreadattributelist
    pub fn InitializeProcThreadAttributeList(
        lpattributelist: LPPROC_THREAD_ATTRIBUTE_LIST,
        dwattributecount: u32,
        dwflags: u32,
        lpsize: *mut usize
    ) -> BOOL {
        unimplemented!()
    }
    // >= Vista / Server 2008
    pub fn UpdateProcThreadAttribute(
        lpattributelist: LPPROC_THREAD_ATTRIBUTE_LIST,
        dwflags: u32,
        attribute: usize,
        lpvalue: *const core::ffi::c_void,
        cbsize: usize,
        lppreviousvalue: *mut core::ffi::c_void,
        lpreturnsize: *const usize
    ) -> BOOL {
        unimplemented!()
    }
    // >= Vista / Server 2008
    pub fn DeleteProcThreadAttributeList(lpattributelist: LPPROC_THREAD_ATTRIBUTE_LIST) {
        unimplemented!()
    }
}
