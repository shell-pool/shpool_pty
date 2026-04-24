mod err;

pub use self::err::DescriptorError;
use std::{
    ffi::CStr,
    os::fd::{FromRawFd, OwnedFd},
};

/// Open the given file as a file descriptor.
pub fn open(
    path: &CStr,
    flag: libc::c_int,
    mode: Option<libc::c_int>,
) -> Result<OwnedFd, DescriptorError> {
    // Safety: we've just ensured that path is non-null and the
    // other params are valid by construction.
    unsafe {
        match libc::open(path.as_ptr().cast(), flag, mode.unwrap_or_default()) {
            -1 => Err(DescriptorError::OpenFail),
            fd => Ok(OwnedFd::from_raw_fd(fd)),
        }
    }
}
