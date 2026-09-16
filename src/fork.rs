mod pty;

pub use self::pty::{Master, MasterError};
pub use self::pty::{Slave, SlaveError};
use crate::descriptor::DescriptorError;
use std::error::Error;
use std::ffi::CStr;
use std::ffi::CString;
use std::fmt;
use std::mem;

/// The alias `Result` learns `ForkError` possibility.
pub type Result<T> = ::std::result::Result<T, ForkError>;

/// The enum `ForkError` defines the possible errors from constructor Fork.
#[derive(Clone, Copy, Debug)]
pub enum ForkError {
    /// Can't creates the child.
    Failure,
    /// Can't set the id group.
    SetsidFail,
    /// Can't make the pty the controlling terminal of the child.
    SetcttyFail,
    /// Can't suspending the calling process.
    WaitpidFail,
    /// Is child and not parent.
    IsChild,
    /// Is parent and not child.
    IsParent,
    /// The Master occured a error.
    BadMaster(MasterError),
    /// The Slave occured a error.
    BadSlave(SlaveError),
    /// The Master's Descriptor occured a error.
    BadDescriptorMaster(DescriptorError),
    /// The Slave's Descriptor occured a error.
    BadDescriptorSlave(DescriptorError),
}

impl fmt::Display for ForkError {
    /// The function `fmt` formats the value using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", ::errno::errno())
    }
}

impl Error for ForkError {
    /// The function `description` returns a short description of the error.
    fn description(&self) -> &str {
        match *self {
            ForkError::Failure => {
                "On failure, -1 is returned in the parent,no child process is created, and errno \
                 isset appropriately."
            }
            ForkError::SetsidFail => {
                "fails if the calling process is alreadya process group leader."
            }
            ForkError::SetcttyFail => {
                "the `TIOCSCTTY` ioctl failed, so the child has no controlling terminal."
            }
            ForkError::WaitpidFail => "Can't suspending the calling process.",
            ForkError::IsChild => "is child and not parent",
            ForkError::IsParent => "is parent and not child",
            ForkError::BadMaster(_) => "the master as occured an error",
            ForkError::BadSlave(_) => "the slave as occured an error",
            ForkError::BadDescriptorMaster(_) => "the master's descriptor as occured an error",
            ForkError::BadDescriptorSlave(_) => "the slave's descriptor as occured an error",
        }
    }

    /// The function `cause` returns the lower-level cause of this error, if
    /// any.
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            ForkError::BadMaster(ref err) => Some(err),
            ForkError::BadSlave(ref err) => Some(err),
            ForkError::BadDescriptorMaster(ref err) => Some(err),
            ForkError::BadDescriptorSlave(ref err) => Some(err),
            _ => None,
        }
    }
}

const MAX_PTS_NAME: usize = 1024;

#[derive(Debug, Clone)]
pub enum Fork {
    // Parent child's pid and master's pty.
    Parent(libc::pid_t, Master),
    // Child pid 0.
    Child(Slave),
}

impl Fork {
    /// The constructor function `new` forks the program
    /// and returns the current pid.
    ///
    /// All the work that might touch the allocator (opening the master and
    /// slave halves of the pty, resolving the name of the slave pty) happens
    /// *before* the `fork(2)` call. Between the fork and the point where this
    /// routine hands control back to the child, the child only makes
    /// async-signal-safe calls (`close(2)`, `setsid(2)`, `ioctl(2)` and
    /// `dup2(2)`).
    ///
    /// This matters because the child of a `fork(2)` in a multithreaded
    /// process may only call async-signal-safe routines until it calls one of
    /// the `exec` family of routines. Only the forking thread survives into
    /// the child, so any lock (the allocator lock in particular) that happened
    /// to be held by one of the other threads at the instant of the fork stays
    /// locked forever, and the child deadlocks the first time it needs it.
    ///
    /// Note that this guarantee only covers what this routine itself does. It
    /// is up to the caller to make sure that whatever it runs in the child
    /// before `exec`ing is async-signal-safe as well.
    pub fn new(path: &'static str) -> Result<Self> {
        // Note the scope: the `CString` has to be freed before we fork,
        // otherwise its destructor would run in the child and call into the
        // allocator there.
        let master = {
            let path = CString::new(path).ok().unwrap_or_default();
            match Master::new(path.as_c_str()) {
                Err(cause) => return Err(ForkError::BadMaster(cause)),
                Ok(master) => master,
            }
        };
        if let Some(cause) = master.grantpt().err().or(master.unlockpt().err()) {
            return Err(ForkError::BadMaster(cause));
        }

        // Look up the name of the slave pty and open it here in the parent so
        // that the child doesn't have to. The buffer lives on the stack rather
        // than the heap for the same reason.
        let mut ptsname_buf = [0u8; MAX_PTS_NAME];
        if let Err(cause) = master.ptsname_r(&mut ptsname_buf) {
            return Err(ForkError::BadMaster(cause));
        }
        let Ok(ptsname) = CStr::from_bytes_until_nul(&ptsname_buf) else {
            // ptsname_r is contracted to null terminate the buffer when it
            // succeeds, so this should never happen.
            return Err(ForkError::BadMaster(MasterError::PtsnameError));
        };
        let slave = match Slave::new_noctty(ptsname) {
            Err(cause) => return Err(ForkError::BadSlave(cause)),
            Ok(slave) => slave,
        };

        // Safety: no params to worry about, just an ffi call
        let fork_ret = unsafe { libc::fork() };
        match fork_ret {
            -1 => Err(ForkError::Failure),
            0 => {
                // Everything from here on out runs in the child, so it has to
                // stay async-signal-safe (see the doc comment above).
                //
                // The child has no use for the master half of the pty, but we
                // can't just drop it, since freeing the `Arc` behind it would
                // mean calling into the allocator. Leak the (tiny, and about
                // to be blown away by an exec anyway) allocation instead and
                // close the fd by hand.
                let master_fd = master.raw_fd();
                mem::forget(master);
                // Safety: the `mem::forget` above means nothing else is going
                // to close this fd, so we are the owner of it.
                unsafe { libc::close(master_fd) };

                Fork::from_slave(slave)
            }
            pid => {
                // The parent has no use for the slave half of the pty, and
                // hanging on to it would keep reads on the master from ever
                // seeing an EOF once the child exits.
                drop(slave);
                Ok(Fork::Parent(pid, master))
            }
        }
    }

    /// The constructor function `from_slave` is a private extension of the
    /// constructor function `new` which runs in the freshly forked child and
    /// wires up the already opened slave pty as the child's controlling
    /// terminal and standard streams.
    ///
    /// Everything it does must be async-signal-safe, see the note on `new`.
    fn from_slave(slave: Slave) -> Result<Self> {
        // Safety: `setsid` takes no arguments, and the fd handed to `ioctl` is
        // owned by `slave`, so it stays valid for the duration of the call.
        unsafe {
            if libc::setsid() == -1 {
                return Err(ForkError::SetsidFail);
            }

            // Now that we are a session leader without a controlling terminal,
            // claim the pty as our controlling terminal. The child used to get
            // this for free by open(2)ing the slave itself, but the open now
            // happens in the parent (with `O_NOCTTY`) so that the child stays
            // async-signal-safe.
            if libc::ioctl(slave.raw_fd(), libc::TIOCSCTTY as _, 0 as libc::c_int) == -1 {
                return Err(ForkError::SetcttyFail);
            }
        }

        if let Some(cause) = slave
            .dup2(libc::STDIN_FILENO)
            .err()
            .or(slave.dup2(libc::STDOUT_FILENO).err())
            .or(slave.dup2(libc::STDERR_FILENO).err())
        {
            return Err(ForkError::BadSlave(cause));
        }

        Ok(Fork::Child(slave))
    }

    /// The constructor function `from_ptmx` forks the program
    /// and returns the current pid for a default PTMX's path.
    pub fn from_ptmx() -> Result<Self> {
        Fork::new(crate::DEFAULT_PTMX)
    }

    /// Waits until it's terminated.
    pub fn wait(&self) -> Result<libc::pid_t> {
        self.wait_for_exit().map(|(p, _)| p)
    }

    /// Waits until it's terminated, returning the exit status if there is one
    pub fn wait_for_exit(&self) -> Result<(libc::pid_t, Option<i32>)> {
        match *self {
            Fork::Child(_) => Err(ForkError::IsChild),
            Fork::Parent(pid, _) => loop {
                unsafe {
                    let mut status = 0;
                    match libc::waitpid(pid, &mut status, 0) {
                        0 => continue,
                        -1 => return Err(ForkError::WaitpidFail),
                        _ => {
                            if libc::WIFEXITED(status) {
                                return Ok((pid, Some(libc::WEXITSTATUS(status))));
                            } else {
                                return Ok((pid, None));
                            }
                        }
                    }
                }
            },
        }
    }

    /// The function `child_pid` returns the pid of the child process if
    /// this instance of Fork represents the parent process and None
    /// in the child process.
    pub fn child_pid(&self) -> Option<libc::pid_t> {
        match *self {
            Fork::Child(_) => None,
            Fork::Parent(pid, _) => Some(pid),
        }
    }

    /// The function `is_parent` returns the pid or parent
    /// or none.
    pub fn is_parent(&self) -> Result<Master> {
        match *self {
            Fork::Child(_) => Err(ForkError::IsChild),
            Fork::Parent(_, ref master) => Ok(master.clone()),
        }
    }

    /// The function `is_child` returns the pid or child
    /// or none.
    pub fn is_child(&self) -> Result<&Slave> {
        match *self {
            Fork::Parent(_, _) => Err(ForkError::IsParent),
            Fork::Child(ref slave) => Ok(slave),
        }
    }
}
