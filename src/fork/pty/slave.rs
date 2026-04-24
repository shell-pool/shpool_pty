use crate::descriptor::{self, DescriptorError};
use std::{
    error::Error,
    ffi::CStr,
    fmt,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
        unix::io::RawFd,
    },
    sync::Arc,
};

/// The alias `Result` learns `SlaveError` possibility.
pub type Result<T> = ::std::result::Result<T, SlaveError>;

/// The enum `SlaveError` defines the possible errors from constructor Slave.
#[derive(Clone, Copy, Debug)]
pub enum SlaveError {
    BadDescriptor(DescriptorError),
    Dup2Error,
}

impl fmt::Display for SlaveError {
    /// The function `fmt` formats the value using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", ::errno::errno())
    }
}

impl Error for SlaveError {
    /// The function `description` returns a short description of the error.
    fn description(&self) -> &str {
        match *self {
            SlaveError::BadDescriptor(_) => "the descriptor as occured an error",
            SlaveError::Dup2Error => "the `dup2` has a error, errno isset appropriately.",
        }
    }

    /// The function `cause` returns the lower-level cause of this error, if any.
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            SlaveError::BadDescriptor(ref err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Slave {
    pty: Arc<OwnedFd>,
}

impl Slave {
    /// The constructor function `new` returns the Slave interface.
    pub fn new(path: &CStr) -> Result<Self> {
        match descriptor::open(path, libc::O_RDWR, None) {
            Err(cause) => Err(SlaveError::BadDescriptor(cause)),
            Ok(fd) => Ok(Slave { pty: Arc::new(fd) }),
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

    pub fn dup2(&self, std: libc::c_int) -> Result<libc::c_int> {
        // Safety: pty is live across the lifetime of this call,
        // so the fd is valid.
        unsafe {
            match libc::dup2(self.raw_fd(), std) {
                -1 => Err(SlaveError::Dup2Error),
                d => Ok(d),
            }
        }
    }
}
