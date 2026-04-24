#[cfg(target_os = "macos")]
mod ptsname_r_macos;

use crate::descriptor::{self, DescriptorError};
use std::{
    error::Error,
    ffi::CStr,
    fmt, io,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
        unix::io::RawFd,
    },
    sync::Arc,
};

/// The alias `Result` learns `MasterError` possibility.
pub type Result<T> = ::std::result::Result<T, MasterError>;

/// The enum `MasterError` defines the possible errors from constructor Master.
#[derive(Clone, Copy, Debug)]
pub enum MasterError {
    BadDescriptor(DescriptorError),
    GrantptError,
    UnlockptError,
    PtsnameError,
}

impl fmt::Display for MasterError {
    /// The function `fmt` formats the value using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", ::errno::errno())
    }
}

impl Error for MasterError {
    /// The function `description` returns a short description of the error.
    fn description(&self) -> &str {
        match *self {
            MasterError::BadDescriptor(_) => "the descriptor as occured an error",
            MasterError::GrantptError => "the `grantpt` has a error, errnois set appropriately.",
            MasterError::UnlockptError => "the `grantpt` has a error, errnois set appropriately.",
            MasterError::PtsnameError => "the `ptsname` has a error",
        }
    }

    /// The function `cause` returns the lower-level cause of this error, if any.
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            MasterError::BadDescriptor(ref err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Master {
    pty: Arc<OwnedFd>,
}

impl Master {
    pub fn new(path: &CStr) -> Result<Self> {
        match descriptor::open(path, libc::O_RDWR, None) {
            Err(cause) => Err(MasterError::BadDescriptor(cause)),
            Ok(fd) => Ok(Master { pty: Arc::new(fd) }),
        }
    }

    /// Extract the raw fd from the underlying object
    pub fn raw_fd(&self) -> RawFd {
        self.pty.as_raw_fd()
    }

    /// Borrow the raw fd
    pub fn borrow_fd(&self) -> BorrowedFd<'_> {
        self.pty.as_fd()
    }

    /// Change UID and GID of slave pty associated with master pty whose
    /// fd is provided, to the real UID and real GID of the calling thread.
    pub fn grantpt(&self) -> Result<libc::c_int> {
        // Safety: pty is live across the lifetime of this call,
        // so the fd is valid.
        unsafe {
            match libc::grantpt(self.raw_fd()) {
                -1 => Err(MasterError::GrantptError),
                c => Ok(c),
            }
        }
    }

    /// Unlock the slave pty associated with the master to which fd refers.
    pub fn unlockpt(&self) -> Result<libc::c_int> {
        // Safety: pty is live across the lifetime of this call,
        // so the fd is valid.
        unsafe {
            match libc::unlockpt(self.raw_fd()) {
                -1 => Err(MasterError::UnlockptError),
                c => Ok(c),
            }
        }
    }

    /// Returns a pointer to a static buffer, which will be overwritten on
    /// subsequent calls.
    pub fn ptsname_r(&self, buf: &mut [u8]) -> Result<()> {
        // Safety: the vector's memory is valid for the duration
        // of the call
        unsafe {
            let data: *mut u8 = &mut buf[0];

            #[cfg(any(target_os = "linux", target_os = "android"))]
            let result = libc::ptsname_r(self.raw_fd(), data as *mut libc::c_char, buf.len());

            #[cfg(target_os = "macos")]
            let result = ptsname_r_macos::ptsname_r(self.raw_fd(), data as *mut libc::c_char, buf.len());

            match result {
                0 => Ok(()),
                _ => Err(MasterError::PtsnameError), // should probably capture errno
            }
        }
    }
}

impl io::Read for Master {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Safety: the vector's memory is valid for the duration
        // of the call
        unsafe {
            match libc::read(
                self.raw_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            ) {
                -1 => Ok(0),
                len => Ok(len as usize),
            }
        }
    }
}

impl io::Write for Master {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            match libc::write(
                self.raw_fd(),
                buf.as_ptr() as *const libc::c_void,
                buf.len(),
            ) {
                -1 => Err(io::Error::last_os_error()),
                ret => Ok(ret as usize),
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
