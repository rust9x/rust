use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut};
use crate::ops::Neg;
use crate::os::windows::prelude::*;
use crate::sync::atomic::Atomic;
use crate::sync::atomic::Ordering::Relaxed;
use crate::sys::handle::Handle;
use crate::sys::{FromInner, IntoInner, api, c};
use crate::{mem, ptr};

pub struct ChildPipe {
    inner: Handle,
}

impl IntoInner<Handle> for ChildPipe {
    fn into_inner(self) -> Handle {
        self.inner
    }
}

impl FromInner<Handle> for ChildPipe {
    fn from_inner(inner: Handle) -> ChildPipe {
        Self { inner }
    }
}

pub(super) struct Pipes {
    pub ours: ChildPipe,
    pub theirs: ChildPipe,
}

/// Creates an anonymous pipe suitable for communication with a child process.
///
/// Windows unfortunately does not have a way of performing asynchronous operations
/// on a handle originally created for synchronous operation. As `read_output` can
/// only be correctly implemented with asynchronous reads but the pipe created by
/// `CreatePipe` is synchronous, we cannot use it (and thus [`io::pipe`]) to create
/// a pipe for communicating with a child. Instead, this function uses the NT API
/// to create a pipe where one pipe handle (`ours`) is asynchronous and one is
/// synchronous and can be inherited by a child for use as a console handle
/// (`theirs`).
///
/// The ours/theirs pipes are *not* specifically readable or writable. Each
/// one only supports a read or a write, but which is which depends on the
/// boolean flag given. If `ours_readable` is `true`, then `ours` is readable and
/// `theirs` is writable. Conversely, if `ours_readable` is `false`, then `ours`
/// is writable and `theirs` is readable.
///
/// Also note that the `ours` pipe is always a handle opened up in overlapped
/// mode. This means that technically speaking it should only ever be used
/// with `OVERLAPPED` instances, but also works out ok if it's only ever used
/// once at a time (which we do indeed guarantee).
// FIXME(joboet): No, we don't guarantee that? E.g. `&Stdout` is both `Read`
//                and `Sync`, so there could be multiple operations at the same
//                time. All the functions below that forward to the inner handle
//                methods could abort if used concurrently.
pub(super) fn child_pipe(ours_readable: bool, their_handle_inheritable: bool) -> io::Result<Pipes> {
    // A 64kb pipe capacity is the same as a typical Linux default.
    const PIPE_BUFFER_CAPACITY: u32 = 64 * 1024;

    // Since Windows 9X/ME does not support creating named pipes (only connecting to remote pipes
    // created on NT), we'll have to make do with anonymous pipes, without overlapped I/O. In
    // particular, this means that we'll have to do reading from two threads in the case where both
    // stdout and stderr being piped (see `read_output`).

    // 9X/ME *does* have a kernel32 export entry for `CreateNamedPipe`, so an availability check
    // would not work. We're just gonna check the bit that's only set on non-unicode Windows
    // versions instead...

    // The `ChildPipe` impl used in `read_output` below needs to be able to cancel the overlapped i/o
    // operation, so we also have to check for `CancelIo` being available. This means that the
    // "modern" path is taken only for NT4+.
    #[cfg(target_family = "rust9x")]
    if !crate::sys::compat::checks::supports_async_io() {
        let size = mem::size_of::<c::SECURITY_ATTRIBUTES>();
        let mut sa = c::SECURITY_ATTRIBUTES {
            nLength: size as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            // We follow the old "Creating a Child Process with Redirected Input and Output" MSDN
            // entry (pre-`SetHandleInformation`) here, duplicating the handle that is not being
            // sent to the child process as non-inheritable and then closing the inheritable one.
            // Usually, this would be racy, but this function is only called in `Stdio::to_handle`,
            // which is in turn only called form `process::spawn`, which acquires a lock on process
            // spawning because of this.
            bInheritHandle: c::TRUE,
        };

        unsafe {
            let mut read_pipe = mem::zeroed();
            let mut write_pipe = mem::zeroed();
            crate::sys::cvt(c::CreatePipe(
                &mut read_pipe,
                &mut write_pipe,
                &mut sa,
                PIPE_BUFFER_CAPACITY,
            ))?;
            let read_pipe = Handle::from_raw_handle(read_pipe);
            let write_pipe = Handle::from_raw_handle(write_pipe);

            let (ours_inheritable, theirs) =
                if ours_readable { (read_pipe, write_pipe) } else { (write_pipe, read_pipe) };

            // Make `ours` non-inheritable by duplicating it with the approriate setting
            let ours = ours_inheritable.duplicate(0, false, c::DUPLICATE_SAME_ACCESS)?;

            // close the old, inheritable handle to the pipe end that is ours
            drop(ours_inheritable);

            return Ok(Pipes {
                ours: ChildPipe { inner: ours },
                theirs: ChildPipe { inner: theirs },
            });
        }
    }

    // based on old Rust implementation from before https://github.com/rust-lang/rust/pull/142517:
    // - https://github.com/rust-lang/rust/blob/64033a4ee541c3e9c178fd593e979c74bb798cdc/library/std/src/sys/pal/windows/pipe.rs
    // - https://github.com/rust-lang/rust/commit/3371d498b1d6853598b9bedaf1e71d4c3f1d538b:
    //   `PIPE_REJECT_REMOTE_CLIENTS` was added with Vista, so we don't need to try it in this
    //   fallback for pre-Vista systems.
    //
    #[cfg(target_family = "rust9x")]
    if !crate::sys::compat::checks::supports_anon_pipe_autoname() {
        fn pipe_serial_number() -> usize {
            use crate::sync::atomic::{Atomic, AtomicUsize, Ordering};
            static N: Atomic<usize> = AtomicUsize::new(0);
            return N.fetch_add(1, Ordering::Relaxed);
        }

        unsafe {
            use crate::ffi::OsStr;
            use crate::sys::fs::{File, OpenOptions};
            use crate::sys::path::WCStr;

            let ours;
            let name = format!(
                r"\\.\pipe\__rust_pipe{}.{}.{}",
                c::GetCurrentProcessId(),
                c::GetTickCount(),
                pipe_serial_number(),
            );
            let wide_name = OsStr::new(&name).encode_wide().chain(Some(0)).collect::<Vec<_>>();
            let wide_name = WCStr::from_wchars_with_null_unchecked(&wide_name);
            let mut flags = c::FILE_FLAG_FIRST_PIPE_INSTANCE | c::FILE_FLAG_OVERLAPPED;
            if ours_readable {
                flags |= c::PIPE_ACCESS_INBOUND;
            } else {
                flags |= c::PIPE_ACCESS_OUTBOUND;
            }

            let handle = c::CreateNamedPipeW(
                wide_name.as_ptr(),
                flags,
                c::PIPE_TYPE_BYTE | c::PIPE_READMODE_BYTE | c::PIPE_WAIT,
                1,
                PIPE_BUFFER_CAPACITY,
                PIPE_BUFFER_CAPACITY,
                0,
                ptr::null_mut(),
            );
            if handle == c::INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }

            ours = Handle::from_raw_handle(handle);

            // Connect to the named pipe we just created. This handle is going to be
            // returned in `theirs`, so if `ours` is readable we want this to be
            // writable, otherwise if `ours` is writable we want this to be
            // readable.
            //
            // Additionally we don't enable overlapped mode on this because most
            // client processes aren't enabled to work with that.
            let mut opts = OpenOptions::new();
            opts.write(ours_readable);
            opts.read(!ours_readable);
            opts.share_mode(0);
            opts.inherit_handle(true);
            let theirs = File::open_native(wide_name, &opts)?;

            return Ok(Pipes {
                ours: ChildPipe { inner: ours },
                theirs: ChildPipe { inner: theirs.into_inner() },
            });
        }
    }

    // Note that we specifically do *not* use `CreatePipe` here because
    // unfortunately the anonymous pipes returned do not support overlapped
    // operations. Instead, we use `NtCreateNamedPipeFile` to create the
    // anonymous pipe with overlapped support.
    //
    // Once we do this, we connect to it via `NtOpenFile`, and then
    // we return those reader/writer halves. Note that the `ours` pipe return
    // value is always the named pipe, whereas `theirs` is just the normal file.
    // This should hopefully shield us from child processes which assume their
    // stdout is a named pipe, which would indeed be odd!
    unsafe {
        let mut io_status = c::IO_STATUS_BLOCK::default();
        let mut object_attributes = c::OBJECT_ATTRIBUTES::default();
        object_attributes.Length = size_of::<c::OBJECT_ATTRIBUTES>() as u32;

        // Open a handle to the pipe filesystem (`\Device\NamedPipe\`).
        // This will be used when creating a new anonymous pipe.
        //
        // We cache the handle once so we can reuse it without needing to reopen it each time.
        // NOTE: this means the handle may appear to be leaked but that's fine because
        // it's only one handle and the OS will clean it up when the process exits.
        static PIPE_FS: Atomic<c::HANDLE> = Atomic::<c::HANDLE>::new(ptr::null_mut());
        let pipe_fs = if let handle = PIPE_FS.load(Relaxed)
            && !handle.is_null()
        {
            handle
        } else {
            let path = api::unicode_str!(r"\Device\NamedPipe\");
            object_attributes.ObjectName = path.as_ptr();
            let mut pipe_fs = ptr::null_mut();
            let status = c::NtOpenFile(
                &mut pipe_fs,
                c::SYNCHRONIZE | c::GENERIC_READ,
                &object_attributes,
                &mut io_status,
                c::FILE_SHARE_READ | c::FILE_SHARE_WRITE,
                c::FILE_SYNCHRONOUS_IO_NONALERT, // synchronous access
            );
            if c::nt_success(status) {
                match PIPE_FS.compare_exchange(ptr::null_mut(), pipe_fs, Relaxed, Relaxed) {
                    Ok(_) => pipe_fs,
                    Err(existing) => {
                        c::CloseHandle(pipe_fs);
                        existing
                    }
                }
            } else {
                return Err(io::Error::from_raw_os_error(c::RtlNtStatusToDosError(status) as i32));
            }
        };

        // From now on we're using handles instead of paths to create and open pipes.
        // So set the `ObjectName` to a zero length string.
        // As a (perhaps overzealous) mitigation for #143078, we use the null pointer
        // for empty.Buffer instead of unicode_str!("").
        // There's no difference to the OS itself but it's possible that third party
        // DLLs which hook in to processes could be relying on the exact form of this string.
        let empty = c::UNICODE_STRING::default();
        object_attributes.ObjectName = &raw const empty;

        // Create our side of the pipe for async access.
        let ours = {
            // Use the pipe filesystem as the root directory.
            // With no name provided, an anonymous pipe will be created.
            object_attributes.RootDirectory = pipe_fs;

            // A negative timeout value is a relative time (rather than an absolute time).
            // The time is given in 100's of nanoseconds so this is 50 milliseconds.
            // This value was chosen to be consistent with the default timeout set by `CreateNamedPipeW`
            // See: https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew
            let timeout = (50_i64 * 10000).neg() as u64;

            let mut ours = ptr::null_mut();
            let status = c::NtCreateNamedPipeFile(
                &mut ours,
                c::SYNCHRONIZE | if ours_readable { c::GENERIC_READ } else { c::GENERIC_WRITE },
                &object_attributes,
                &mut io_status,
                if ours_readable { c::FILE_SHARE_WRITE } else { c::FILE_SHARE_READ },
                c::FILE_CREATE,
                0,
                c::FILE_PIPE_BYTE_STREAM_TYPE,
                c::FILE_PIPE_BYTE_STREAM_MODE,
                c::FILE_PIPE_QUEUE_OPERATION,
                // only allow one client pipe
                1,
                PIPE_BUFFER_CAPACITY,
                PIPE_BUFFER_CAPACITY,
                &timeout,
            );
            if c::nt_success(status) {
                Handle::from_raw_handle(ours)
            } else {
                return Err(io::Error::from_raw_os_error(c::RtlNtStatusToDosError(status) as i32));
            }
        };

        // Open their side of the pipe for synchronous access.
        let theirs = {
            // We can reopen the anonymous pipe without a name by setting
            // RootDirectory to the pipe handle and not setting a path name,
            object_attributes.RootDirectory = ours.as_raw_handle();

            if their_handle_inheritable {
                object_attributes.Attributes |= c::OBJ_INHERIT;
            }
            let mut theirs = ptr::null_mut();
            let status = c::NtOpenFile(
                &mut theirs,
                c::SYNCHRONIZE
                    | if ours_readable {
                        c::GENERIC_WRITE | c::FILE_READ_ATTRIBUTES
                    } else {
                        c::GENERIC_READ
                    },
                &object_attributes,
                &mut io_status,
                0,
                c::FILE_NON_DIRECTORY_FILE | c::FILE_SYNCHRONOUS_IO_NONALERT,
            );
            if c::nt_success(status) {
                Handle::from_raw_handle(theirs)
            } else {
                return Err(io::Error::from_raw_os_error(c::RtlNtStatusToDosError(status) as i32));
            }
        };

        Ok(Pipes { ours: ChildPipe { inner: ours }, theirs: ChildPipe { inner: theirs } })
    }
}

/// Takes an asynchronous source pipe and returns a synchronous pipe suitable
/// for sending to a child process.
///
/// This is achieved by creating a new set of pipes and spawning a thread that
/// relays messages between the source and the synchronous pipe.
pub(super) fn spawn_pipe_relay(
    source: &ChildPipe,
    ours_readable: bool,
    their_handle_inheritable: bool,
) -> io::Result<ChildPipe> {
    // We need this handle to live for the lifetime of the thread spawned below.
    let source = source.try_clone()?;

    // create a new pair of anon pipes.
    let Pipes { theirs, ours } = child_pipe(ours_readable, their_handle_inheritable)?;

    // Spawn a thread that passes messages from one pipe to the other.
    // Any errors will simply cause the thread to exit.
    let (reader, writer) = if ours_readable { (ours, source) } else { (source, ours) };
    crate::thread::spawn(move || {
        let mut buf = [0_u8; 4096];
        'reader: while let Ok(len) = reader.read(&mut buf) {
            if len == 0 {
                break;
            }
            let mut start = 0;
            while let Ok(written) = writer.write(&buf[start..len]) {
                start += written;
                if start == len {
                    continue 'reader;
                }
            }
            break;
        }
    });

    // Return the pipe that should be sent to the child process.
    Ok(theirs)
}

impl ChildPipe {
    pub fn handle(&self) -> &Handle {
        &self.inner
    }
    pub fn into_handle(self) -> Handle {
        self.inner
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        self.inner.duplicate(0, false, c::DUPLICATE_SAME_ACCESS).map(|inner| ChildPipe { inner })
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_family = "rust9x")]
        if !crate::sys::compat::checks::supports_async_io() {
            return self.inner.read(buf);
        }

        let result = unsafe {
            let len = crate::cmp::min(buf.len(), u32::MAX as usize) as u32;
            let ptr = buf.as_mut_ptr();
            self.alertable_io_internal(|overlapped, callback| {
                c::ReadFileEx(self.inner.as_raw_handle(), ptr, len, overlapped, callback)
            })
        };

        match result {
            // The special treatment of BrokenPipe is to deal with Windows
            // pipe semantics, which yields this error when *reading* from
            // a pipe after the other end has closed; we interpret that as
            // EOF on the pipe.
            Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(0),
            _ => result,
        }
    }

    pub fn read_buf(&self, mut buf: BorrowedCursor<'_, u8>) -> io::Result<()> {
        #[cfg(target_family = "rust9x")]
        if !crate::sys::compat::checks::supports_async_io() {
            return self.inner.read_buf(buf);
        }

        let result = unsafe {
            let len = crate::cmp::min(buf.capacity(), u32::MAX as usize) as u32;
            let ptr = buf.as_mut().as_mut_ptr().cast::<u8>();
            self.alertable_io_internal(|overlapped, callback| {
                c::ReadFileEx(self.inner.as_raw_handle(), ptr, len, overlapped, callback)
            })
        };

        match result {
            // The special treatment of BrokenPipe is to deal with Windows
            // pipe semantics, which yields this error when *reading* from
            // a pipe after the other end has closed; we interpret that as
            // EOF on the pipe.
            Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            Err(e) => Err(e),
            Ok(n) => {
                unsafe {
                    buf.advance(n);
                }
                Ok(())
            }
        }
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        self.inner.is_read_vectored()
    }

    pub fn read_to_end(&self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.handle().read_to_end(buf)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        #[cfg(target_family = "rust9x")]
        if !crate::sys::compat::checks::supports_async_io() {
            return self.inner.write(buf);
        }

        unsafe {
            let len = crate::cmp::min(buf.len(), u32::MAX as usize) as u32;
            self.alertable_io_internal(|overlapped, callback| {
                c::WriteFileEx(self.inner.as_raw_handle(), buf.as_ptr(), len, overlapped, callback)
            })
        }
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    /// Synchronizes asynchronous reads or writes using our anonymous pipe.
    ///
    /// This is a wrapper around [`ReadFileEx`] or [`WriteFileEx`] that uses
    /// [Asynchronous Procedure Call] (APC) to synchronize reads or writes.
    ///
    /// Note: This should not be used for handles we don't create.
    ///
    /// # Safety
    ///
    /// `buf` must be a pointer to a buffer that's valid for reads or writes
    /// up to `len` bytes. The `AlertableIoFn` must be either `ReadFileEx` or `WriteFileEx`
    ///
    /// [`ReadFileEx`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-readfileex
    /// [`WriteFileEx`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefileex
    /// [Asynchronous Procedure Call]: https://docs.microsoft.com/en-us/windows/win32/sync/asynchronous-procedure-calls
    unsafe fn alertable_io_internal(
        &self,
        io: impl FnOnce(&mut c::OVERLAPPED, c::LPOVERLAPPED_COMPLETION_ROUTINE) -> c::BOOL,
    ) -> io::Result<usize> {
        // Use "alertable I/O" to synchronize the pipe I/O.
        // This has four steps.
        //
        // STEP 1: Start the asynchronous I/O operation.
        //         This simply calls either `ReadFileEx` or `WriteFileEx`,
        //         giving it a pointer to the buffer and callback function.
        //
        // STEP 2: Enter an alertable state.
        //         The callback set in step 1 will not be called until the thread
        //         enters an "alertable" state. This can be done using `SleepEx`.
        //
        // STEP 3: The callback
        //         Once the I/O is complete and the thread is in an alertable state,
        //         the callback will be run on the same thread as the call to
        //         `ReadFileEx` or `WriteFileEx` done in step 1.
        //         In the callback we simply set the result of the async operation.
        //
        // STEP 4: Return the result.
        //         At this point we'll have a result from the callback function
        //         and can simply return it. Note that we must not return earlier,
        //         while the I/O is still in progress.

        // The result that will be set from the asynchronous callback.
        let mut async_result: Option<AsyncResult> = None;
        struct AsyncResult {
            error: u32,
            transferred: u32,
        }

        // STEP 3: The callback.
        unsafe extern "system" fn callback(
            error: u32,
            transferred: u32,
            overlapped: *mut c::OVERLAPPED,
        ) {
            // Set `async_result` using a pointer smuggled through `hEvent`.
            // SAFETY:
            // At this point, the OVERLAPPED struct will have been written to by the OS,
            // except for our `hEvent` field which we set to a valid AsyncResult pointer (see below)
            unsafe {
                let result = AsyncResult { error, transferred };
                *(*overlapped).hEvent.cast::<Option<AsyncResult>>() = Some(result);
            }
        }

        // STEP 1: Start the I/O operation.
        let mut overlapped: c::OVERLAPPED = unsafe { crate::mem::zeroed() };
        // `hEvent` is unused by `ReadFileEx` and `WriteFileEx`.
        // Therefore the documentation suggests using it to smuggle a pointer to the callback.
        overlapped.hEvent = (&raw mut async_result) as *mut _;

        // Asynchronous read of the pipe.
        // If successful, `callback` will be called once it completes.
        let result = io(&mut overlapped, Some(callback));
        if result == c::FALSE {
            // We can return here because the call failed.
            // After this we must not return until the I/O completes.
            return Err(io::Error::last_os_error());
        }

        // Wait indefinitely for the result.
        let result = loop {
            // STEP 2: Enter an alertable state.
            // The second parameter of `SleepEx` is used to make this sleep alertable.
            unsafe { c::SleepEx(c::INFINITE, c::TRUE) };
            if let Some(result) = async_result {
                break result;
            }
        };
        // STEP 4: Return the result.
        // `async_result` is always `Some` at this point
        match result.error {
            c::ERROR_SUCCESS => Ok(result.transferred as usize),
            error => Err(io::Error::from_raw_os_error(error as _)),
        }
    }
}

pub fn read_output(
    p1: ChildPipe,
    v1: &mut Vec<u8>,
    p2: ChildPipe,
    v2: &mut Vec<u8>,
) -> io::Result<()> {
    let p1 = p1.into_handle();
    let p2 = p2.into_handle();

    #[cfg(target_family = "rust9x")]
    if !crate::sys::compat::checks::supports_async_io() {
        // Since we are using anonymous pipes (= without overlapped I/O support) here, we can't do
        // async waiting on both stdout and stderr at the same time on one thread, so we have to
        // spawn an additional thread to do the waiting for the second pipe.

        // See https://github.com/rust-lang/rust/pull/31618, where this was removed initially.
        let second_pipe = crate::thread::spawn(move || {
            let mut ret = Vec::new();
            (&p2).read_to_end(&mut ret).map(|_| ret)
        });

        (&p1).read_to_end(v1)?;
        *v2 = second_pipe.join().unwrap()?;

        return Ok(());
    }

    let mut p1 = AsyncPipe::new(p1, v1)?;
    let mut p2 = AsyncPipe::new(p2, v2)?;
    let objs = [p1.event.as_raw_handle(), p2.event.as_raw_handle()];

    // In a loop we wait for either pipe's scheduled read operation to complete.
    // If the operation completes with 0 bytes, that means EOF was reached, in
    // which case we just finish out the other pipe entirely.
    //
    // Note that overlapped I/O is in general super unsafe because we have to
    // be careful to ensure that all pointers in play are valid for the entire
    // duration of the I/O operation (where tons of operations can also fail).
    // The destructor for `AsyncPipe` ends up taking care of most of this.
    loop {
        let res = unsafe { c::WaitForMultipleObjects(2, objs.as_ptr(), c::FALSE, c::INFINITE) };
        if res == c::WAIT_OBJECT_0 {
            if !p1.result()? || !p1.schedule_read()? {
                return p2.finish();
            }
        } else if res == c::WAIT_OBJECT_0 + 1 {
            if !p2.result()? || !p2.schedule_read()? {
                return p1.finish();
            }
        } else {
            return Err(io::Error::last_os_error());
        }
    }
}

struct AsyncPipe<'a> {
    pipe: Handle,
    event: Handle,
    overlapped: Box<c::OVERLAPPED>, // needs a stable address
    dst: &'a mut Vec<u8>,
    state: State,
}

#[derive(PartialEq, Debug)]
enum State {
    NotReading,
    Reading,
    Read(usize),
}

impl<'a> AsyncPipe<'a> {
    fn new(pipe: Handle, dst: &'a mut Vec<u8>) -> io::Result<AsyncPipe<'a>> {
        // Create an event which we'll use to coordinate our overlapped
        // operations, this event will be used in WaitForMultipleObjects
        // and passed as part of the OVERLAPPED handle.
        //
        // Note that we do a somewhat clever thing here by flagging the
        // event as being manually reset and setting it initially to the
        // signaled state. This means that we'll naturally fall through the
        // WaitForMultipleObjects call above for pipes created initially,
        // and the only time an even will go back to "unset" will be once an
        // I/O operation is successfully scheduled (what we want).
        let event = Handle::new_event(true, true)?;
        let mut overlapped: Box<c::OVERLAPPED> = unsafe { Box::new(mem::zeroed()) };
        overlapped.hEvent = event.as_raw_handle();
        Ok(AsyncPipe { pipe, overlapped, event, dst, state: State::NotReading })
    }

    /// Executes an overlapped read operation.
    ///
    /// Must not currently be reading, and returns whether the pipe is currently
    /// at EOF or not. If the pipe is not at EOF then `result()` must be called
    /// to complete the read later on (may block), but if the pipe is at EOF
    /// then `result()` should not be called as it will just block forever.
    fn schedule_read(&mut self) -> io::Result<bool> {
        assert_eq!(self.state, State::NotReading);
        let amt = unsafe {
            if self.dst.capacity() == self.dst.len() {
                let additional = if self.dst.capacity() == 0 { 16 } else { 1 };
                self.dst.reserve(additional);
            }
            self.pipe.read_overlapped(self.dst.spare_capacity_mut(), &mut *self.overlapped)?
        };

        // If this read finished immediately then our overlapped event will
        // remain signaled (it was signaled coming in here) and we'll progress
        // down to the method below.
        //
        // Otherwise the I/O operation is scheduled and the system set our event
        // to not signaled, so we flag ourselves into the reading state and move
        // on.
        self.state = match amt {
            Some(0) => return Ok(false),
            Some(amt) => State::Read(amt),
            None => State::Reading,
        };
        Ok(true)
    }

    /// Wait for the result of the overlapped operation previously executed.
    ///
    /// Takes a parameter `wait` which indicates if this pipe is currently being
    /// read whether the function should block waiting for the read to complete.
    ///
    /// Returns values:
    ///
    /// * `true` - finished any pending read and the pipe is not at EOF (keep
    ///            going)
    /// * `false` - finished any pending read and pipe is at EOF (stop issuing
    ///             reads)
    fn result(&mut self) -> io::Result<bool> {
        let amt = match self.state {
            State::NotReading => return Ok(true),
            State::Reading => self.pipe.overlapped_result(&mut *self.overlapped, true)?,
            State::Read(amt) => amt,
        };
        self.state = State::NotReading;
        unsafe {
            let len = self.dst.len();
            self.dst.set_len(len + amt);
        }
        Ok(amt != 0)
    }

    /// Finishes out reading this pipe entirely.
    ///
    /// Waits for any pending and schedule read, and then calls `read_to_end`
    /// if necessary to read all the remaining information.
    fn finish(&mut self) -> io::Result<()> {
        while self.result()? && self.schedule_read()? {
            // ...
        }
        Ok(())
    }
}

impl<'a> Drop for AsyncPipe<'a> {
    fn drop(&mut self) {
        let State::Reading = self.state else { return };

        // If we have a pending read operation, then we have to make sure that
        // it's *done* before we actually drop this type. The kernel requires
        // that the `OVERLAPPED` and buffer pointers are valid for the entire
        // I/O operation.
        //
        // To do that, we call `CancelIo` to cancel any pending operation, and
        // if that succeeds we wait for the overlapped result.
        //
        // If anything here fails, there's not really much we can do, so we leak
        // the buffer/OVERLAPPED pointers to ensure we're at least memory safe.
        if self.pipe.cancel_io().is_err() || self.result().is_err() {
            let buf = mem::take(self.dst);
            let overlapped = Box::new(unsafe { mem::zeroed() });
            let overlapped = mem::replace(&mut self.overlapped, overlapped);
            mem::forget((buf, overlapped));
        }
    }
}
