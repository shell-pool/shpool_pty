use std::{
    error::Error,
    ffi::CStr,
    fmt,
    os::fd::{FromRawFd, OwnedFd},
};

/// The enum `DescriptorError` defines the possible errors
/// from constructor Descriptor.
#[derive(Clone, Copy, Debug)]
pub enum DescriptorError {
    /// Can't open.
    OpenFail,
    /// Can't closed.
    CloseFail,
}

impl fmt::Display for DescriptorError {
    /// The function `fmt` formats the value using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", ::errno::errno())
    }
}

impl Error for DescriptorError {
    /// The function `description` returns a short description of the error.
    fn description(&self) -> &str {
        match *self {
            DescriptorError::OpenFail => "can't open the fd",
            DescriptorError::CloseFail => "can't close the fd",
        }
    }

    /// The function `cause` returns the lower-level cause of this error, if
    /// any.
    fn cause(&self) -> Option<&dyn Error> {
        None
    }
}

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
