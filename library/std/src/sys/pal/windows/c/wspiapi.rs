//! IPv4-only `getaddrinfo`/`freeaddrinfo` fallback on top of `gethostbyname`, loosely based on
//! WSPiApi.h.
//!
//! Only supports the subset that is required for `lookup_host`, everything else fails with
//! `EAI_FAIL`.

use crate::ffi::c_int;
use crate::ptr;
use crate::sys::c::{self, ADDRINFOA, AF_INET, AF_UNSPEC, PCSTR};
use crate::sys::net::netc::{in_addr, sockaddr_in};

const EAI_FAIL: c_int = c::WSANO_RECOVERY;
const PF_INET: i32 = AF_INET as i32;

/// Single allocation per list entry, `ai_addr` points to `addr`.
#[repr(C)]
struct AddrInfo {
    info: ADDRINFOA,
    addr: sockaddr_in,
}

pub unsafe fn wspiapi_freeaddrinfo(mut cur: *const ADDRINFOA) {
    while !cur.is_null() {
        // SAFETY: all entries were allocated as `AddrInfo` by `wspiapi_getaddrinfo`
        let entry = unsafe { Box::from_raw(cur.cast_mut().cast::<AddrInfo>()) };
        cur = entry.info.ai_next;
    }
}

pub unsafe fn wspiapi_getaddrinfo(
    node: PCSTR,
    service: PCSTR,
    hints: *const ADDRINFOA,
    res: *mut *mut ADDRINFOA,
) -> c_int {
    let result = unsafe { lookup(node, service, hints.as_ref()) };
    let (list, error) = match result {
        Ok(list) => (list, 0),
        Err(error) => unsafe {
            c::WSASetLastError(error);
            (ptr::null_mut(), error)
        },
    };
    unsafe { *res = list };
    error
}

unsafe fn lookup(
    node: PCSTR,
    service: PCSTR,
    hints: Option<&ADDRINFOA>,
) -> Result<*mut ADDRINFOA, c_int> {
    if node.is_null() || !service.is_null() {
        return Err(EAI_FAIL);
    }
    let (socktype, protocol) = match hints {
        None => (0, 0),
        Some(h) if h.ai_flags == 0 && matches!(h.ai_family, AF_UNSPEC | PF_INET) => {
            (h.ai_socktype, h.ai_protocol)
        }
        Some(_) => return Err(EAI_FAIL),
    };

    // The returned `hostent` is thread-local winsock storage and must be copied before the next
    // winsock call.
    let Some(host) = (unsafe { c::gethostbyname(node).as_ref() }) else {
        return Err(match unsafe { c::WSAGetLastError() } {
            e @ (c::WSATRY_AGAIN | c::WSANO_RECOVERY) => e,
            _ => c::WSAHOST_NOT_FOUND,
        });
    };
    if host.h_addrtype != AF_INET as i16
        || host.h_length != size_of::<in_addr>() as i16
        || unsafe { (*host.h_addr_list).is_null() }
    {
        return Err(EAI_FAIL);
    }

    let mut list = ptr::null_mut();
    let mut tail: *mut *mut ADDRINFOA = &raw mut list;
    let mut addrs = host.h_addr_list;
    unsafe {
        while !(*addrs).is_null() {
            let entry = Box::into_raw(Box::new(AddrInfo {
                info: ADDRINFOA {
                    ai_family: PF_INET,
                    ai_socktype: socktype,
                    ai_protocol: protocol,
                    ai_addrlen: size_of::<sockaddr_in>(),
                    ..Default::default()
                },
                addr: sockaddr_in {
                    sin_family: AF_INET,
                    sin_port: 0,
                    sin_addr: (*addrs).cast::<in_addr>().read_unaligned(),
                    sin_zero: [0; 8],
                },
            }));
            (*entry).info.ai_addr = (&raw mut (*entry).addr).cast();
            *tail = entry.cast();
            tail = &raw mut (*entry).info.ai_next;
            addrs = addrs.add(1);
        }
    }
    Ok(list)
}
