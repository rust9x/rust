use crate::mem::ManuallyDrop;
use crate::pin::Pin;
use crate::sys::compat::thread_parking::{ThreadParkingImpl, thread_parking_impl};
use crate::time::Duration;

pub union Parker {
    futex: ManuallyDrop<super::futex::Parker>,
    keyed_event: ManuallyDrop<super::windowsxp::Parker>,
    generic: ManuallyDrop<super::generic::Parker>,
}

impl Parker {
    pub unsafe fn new_in_place(parker: *mut Parker) {
        let impl_ = thread_parking_impl();

        match impl_ {
            ThreadParkingImpl::Futex => unsafe {
                super::futex::Parker::new_in_place(&raw mut (*(*parker).futex))
            },
            ThreadParkingImpl::KeyedEvent => unsafe {
                super::windowsxp::Parker::new_in_place(&raw mut (*(*parker).keyed_event))
            },
            ThreadParkingImpl::Generic => unsafe {
                super::generic::Parker::new_in_place(&raw mut (*(*parker).generic))
            },
        }
    }

    pub unsafe fn park(self: Pin<&Self>) {
        let impl_ = thread_parking_impl();

        match impl_ {
            ThreadParkingImpl::Futex => unsafe { self.map_unchecked(|p| &*p.futex).park() },
            ThreadParkingImpl::KeyedEvent => unsafe {
                self.map_unchecked(|p| &*p.keyed_event).park()
            },
            ThreadParkingImpl::Generic => unsafe { self.map_unchecked(|p| &*p.generic).park() },
        }
    }

    pub unsafe fn park_timeout(self: Pin<&Self>, timeout: Duration) {
        let impl_ = thread_parking_impl();

        match impl_ {
            ThreadParkingImpl::Futex => unsafe {
                self.map_unchecked(|p| &*p.futex).park_timeout(timeout)
            },
            ThreadParkingImpl::KeyedEvent => unsafe {
                self.map_unchecked(|p| &*p.keyed_event).park_timeout(timeout)
            },
            ThreadParkingImpl::Generic => unsafe {
                self.map_unchecked(|p| &*p.generic).park_timeout(timeout)
            },
        }
    }

    pub fn unpark(self: Pin<&Self>) {
        let impl_ = thread_parking_impl();

        match impl_ {
            ThreadParkingImpl::Futex => unsafe {
                self.map_unchecked(|p| &*p.futex).unpark();
            },
            ThreadParkingImpl::KeyedEvent => unsafe {
                self.map_unchecked(|p| &*p.keyed_event).unpark()
            },
            ThreadParkingImpl::Generic => unsafe { self.map_unchecked(|p| &*p.generic).unpark() },
        }
    }
}

impl Drop for Parker {
    fn drop(&mut self) {
        let impl_ = thread_parking_impl();

        match impl_ {
            ThreadParkingImpl::Futex | ThreadParkingImpl::KeyedEvent => {
                // these don't require any cleanup
            }
            ThreadParkingImpl::Generic => unsafe { ManuallyDrop::drop(&mut self.generic) },
        }
    }
}
