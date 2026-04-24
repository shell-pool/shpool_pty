mod err;

use crate::descriptor;

pub use self::err::{Result, SlaveError};

use std::{
    ffi::CStr,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
        unix::io::RawFd,
    },
    sync::Arc,
};

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
