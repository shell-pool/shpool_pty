//! The child used to pick up the pty as its controlling terminal implicitly,
//! by being the one to `open(2)` the slave. It now opens the slave in the
//! parent (with `O_NOCTTY`) so that the child stays async-signal-safe, and the
//! child claims the terminal explicitly with a `TIOCSCTTY` ioctl instead.
//! Check that the child really does end up with the pty as its controlling
//! terminal, since job control in the spawned program depends on it.

extern crate libc;
extern crate shpool_pty;

use self::shpool_pty::prelude::*;

/// The child exits with one of these if something is off, so that the parent
/// can report which check failed.
const NOT_SESSION_LEADER: i32 = 1;
const NO_CONTROLLING_TERMINAL: i32 = 2;
const PTY_IS_NOT_THE_CONTROLLING_TERMINAL: i32 = 3;

#[test]
fn it_sets_the_controlling_terminal() {
    let fork = Fork::from_ptmx().expect("failed to fork");

    if fork.is_child().is_ok() {
        // Safety: basic ffi. Note that we `_exit` rather than returning, so
        // that the test harness doesn't run the rest of the suite over again
        // in the child.
        unsafe {
            if libc::getsid(0) != libc::getpid() {
                libc::_exit(NOT_SESSION_LEADER);
            }

            // Opening /dev/tty only works for a process that has a
            // controlling terminal.
            if libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR) == -1 {
                libc::_exit(NO_CONTROLLING_TERMINAL);
            }

            // ...and `tcgetsid` only returns a session for a terminal which
            // is the controlling terminal of that session, so this pins down
            // that the controlling terminal is the pty we were handed rather
            // than something we inherited from the parent.
            if libc::tcgetsid(libc::STDIN_FILENO) != libc::getpid() {
                libc::_exit(PTY_IS_NOT_THE_CONTROLLING_TERMINAL);
            }

            libc::_exit(0);
        }
    }

    let (_, status) = fork.wait_for_exit().expect("failed to wait for the child");
    match status {
        Some(0) => (),
        Some(NOT_SESSION_LEADER) => panic!("the child is not a session leader"),
        Some(NO_CONTROLLING_TERMINAL) => panic!("the child has no controlling terminal"),
        Some(PTY_IS_NOT_THE_CONTROLLING_TERMINAL) => {
            panic!("the child's pty is not its controlling terminal")
        }
        Some(code) => panic!("the child exited with an unexpected status: {code}"),
        None => panic!("the child did not exit normally"),
    }
}
