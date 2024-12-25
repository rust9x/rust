//! Manual implementation of a subset of `GetFileInformationByHandleEx` and
//! `SetFileInformationByHandle` based on `fileextd.lib` from Microsoft. Many of their later-added
//! APIs do exist all the way back to NT3.1, but were not exposed via a Win32 API until later.

use super::*;

pub(crate) unsafe fn get_file_information_by_handle_ex(
    file_handle: HANDLE,
    fileinformationclass: FILE_INFO_BY_HANDLE_CLASS,
    lpfileinformation: *mut core::ffi::c_void,
    dwbuffersize: u32,
) -> BOOL {
    if !crate::sys::compat::checks::is_windows_nt() {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED) };
        return FALSE;
    }

    unsafe {
        #[allow(non_upper_case_globals)]
        let (class, min_buffer_size, use_directory_api_info): (
            FILE_INFORMATION_CLASS,
            u32,
            Option<bool>,
        ) = match fileinformationclass {
            FileBasicInfo => {
                (FileBasicInformation, core::mem::size_of::<FILE_BASIC_INFO>() as u32, None)
            }
            FileNameInfo => {
                (FileNameInformation, core::mem::size_of::<FILE_NAME_INFO>() as u32, None)
            }
            FileAttributeTagInfo => (
                // NT API: 2000 and up
                FileAttributeTagInformation,
                core::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
                None,
            ),
            // rust9x: replaced with the following two
            // FileIdBothDirectoryInfo => (
            //     // NT API: XP/2003 and up; exposed via Win32 API since Vista
            //     FileIdBothDirectoryInformation,
            //     core::mem::size_of::<FILE_ID_BOTH_DIR_INFO>() as u32,
            //     Some(false),
            // ),
            // FileIdBothDirectoryRestartInfo => (
            //     // NT API: XP/2003 and up; exposed via Win32 API since Vista
            //     FileIdBothDirectoryInformation,
            //     core::mem::size_of::<FILE_ID_BOTH_DIR_INFO>() as u32,
            //     Some(true),
            // ),
            FileFullDirectoryInfo => (
                // NT API: at least NT4 but likely 3.1+; exposed via Win32 API since Win8
                FileFullDirectoryInformation,
                core::mem::size_of::<FILE_FULL_DIR_INFO>() as u32,
                Some(false),
            ),
            FileFullDirectoryRestartInfo => (
                // NT API: at least NT4 but likely 3.1+; exposed via Win32 API since Win8
                FileFullDirectoryInformation,
                core::mem::size_of::<FILE_FULL_DIR_INFO>() as u32,
                Some(true),
            ),
            _ => {
                SetLastError(ERROR_INVALID_PARAMETER);
                return FALSE;
            }
        };

        if dwbuffersize < min_buffer_size {
            SetLastError(ERROR_BAD_LENGTH);
            return FALSE;
        }

        let mut io_status_block: IO_STATUS_BLOCK = core::mem::zeroed();
        let mut status = if let Some(restart_scan) = use_directory_api_info {
            NtQueryDirectoryFile(
                file_handle,
                core::ptr::null_mut(),
                None,
                core::ptr::null(),
                &mut io_status_block,
                lpfileinformation,
                dwbuffersize,
                class,
                false as BOOLEAN,
                core::ptr::null(),
                restart_scan as BOOLEAN,
            )
        } else {
            NtQueryInformationFile(
                file_handle,
                &mut io_status_block,
                lpfileinformation,
                dwbuffersize,
                class,
            )
        };

        if status == STATUS_PENDING {
            status = NtWaitForSingleObject(file_handle, false as BOOLEAN, core::ptr::null_mut());
        }

        if status < 0 {
            set_last_error_from_ntstatus(status);
            return FALSE;
        }

        // keeping for reference; FileStreamInformation is not used by the standard library
        // currently if class == FileStreamInformation && io_status_block.Information == 0 {
        //     set_last_error_from_ntstatus(STATUS_END_OF_FILE); return FALSE; }
    }
    TRUE
}

pub(crate) unsafe fn set_file_information_by_handle(
    file_handle: HANDLE,
    fileinformationclass: FILE_INFO_BY_HANDLE_CLASS,
    lpfileinformation: *const core::ffi::c_void,
    dwbuffersize: u32,
) -> BOOL {
    if !crate::sys::compat::checks::is_windows_nt() {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED) };
        return FALSE;
    }

    unsafe {
        #[allow(non_upper_case_globals)]
        let (class, min_buffer_size): (FILE_INFORMATION_CLASS, u32) = match fileinformationclass {
            FileBasicInfo => (FileBasicInformation, core::mem::size_of::<FILE_BASIC_INFO>() as u32),
            FileEndOfFileInfo => {
                (FileEndOfFileInformation, core::mem::size_of::<FILE_END_OF_FILE_INFO>() as u32)
            }
            FileDispositionInfo => {
                (FileDispositionInformation, core::mem::size_of::<FILE_DISPOSITION_INFO>() as u32)
            }
            FileDispositionInfoEx => (
                // NT API: some Windows 10 version
                FileDispositionInformationEx,
                core::mem::size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
            ),
            _ => {
                SetLastError(ERROR_INVALID_PARAMETER);
                return FALSE;
            }
        };

        if dwbuffersize < min_buffer_size {
            SetLastError(ERROR_BAD_LENGTH);
            return FALSE;
        }

        let mut io_status_block: IO_STATUS_BLOCK = core::mem::zeroed();
        let status = NtSetInformationFile(
            file_handle,
            &mut io_status_block,
            lpfileinformation,
            dwbuffersize,
            class,
        );

        if status < 0 {
            set_last_error_from_ntstatus(status);
            return FALSE;
        }
    }

    TRUE
}

fn set_last_error_from_ntstatus(status: NTSTATUS) {
    unsafe {
        let error = RtlNtStatusToDosError(status);
        SetLastError(error);
    }
}
