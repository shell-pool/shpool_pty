mod err;

use descriptor::Descriptor;
use libc;

pub use self::err::{Result, SlaveError};
use std::ffi::CStr;
use std::os::fd::BorrowedFd;
use std::os::unix::io::RawFd;

#[derive(Debug, Clone)]
pub struct Slave {
    pty: Option<RawFd>,
}

impl Slave {
    /// The constructor function `new` returns the Slave interface.
    pub fn new(path: &CStr) -> Result<Self> {
        match Self::open(path, libc::O_RDWR, None) {
            Err(cause) => Err(SlaveError::BadDescriptor(cause)),
            Ok(fd) => Ok(Slave { pty: Some(fd) }),
        }
    }

    /// Extract the raw fd from the underlying object
    pub fn raw_fd(&self) -> &Option<RawFd> {
        &self.pty
    }

    /// Borrow the raw fd
    pub fn borrow_fd(&self) -> Option<BorrowedFd<'_>> {
        // Safety: we only ever close on drop, so this will be
        //         live the whole time.
        self.pty.map(|fd| unsafe { BorrowedFd::borrow_raw(fd) })
    }

    pub fn dup2(&self, std: libc::c_int) -> Result<libc::c_int> {
        if let Some(fd) = self.pty {
            unsafe {
                match libc::dup2(fd, std) {
                    -1 => Err(SlaveError::Dup2Error),
                    d => Ok(d),
                }
            }
        } else {
            Err(SlaveError::NoFdError)
        }
    }
}

unsafe impl Descriptor for Slave {
    fn take_raw_fd(&mut self) -> Option<RawFd> {
        self.pty.take()
    }
}

impl Drop for Slave {
    fn drop(&mut self) {
        Descriptor::drop(self);
    }
}
