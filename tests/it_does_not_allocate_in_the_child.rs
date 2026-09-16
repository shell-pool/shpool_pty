//! Regression test for the async-signal-safety of the child half of
//! `Fork::new`.
//!
//! The child of a `fork(2)` in a multithreaded process may only call
//! async-signal-safe routines until it `exec`s, since only the forking thread
//! survives into the child and any lock held by one of the other threads stays
//! locked forever. Calling into the allocator is the easiest way to violate
//! that rule, so this test installs a global allocator which blows the child
//! up (rather than deadlocking, which is what happens in the wild) if anything
//! reaches for the heap between `fork(2)` returning and `Fork::new` handing
//! control back to us.

extern crate libc;
extern crate shpool_pty;

use self::shpool_pty::prelude::*;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, Ordering},
};

/// The exit code the child uses when the library touched the allocator.
const ALLOCATED_EXIT_CODE: i32 = 42;

/// The exit code the child uses when the fork itself failed.
const FORK_FAILED_EXIT_CODE: i32 = 43;

/// True only in a freshly forked child, between `fork(2)` returning and the
/// test clearing it again.
static IN_FORKED_CHILD: AtomicBool = AtomicBool::new(false);

#[global_allocator]
static ALLOCATOR: TrapAllocator = TrapAllocator;

/// A pass-through allocator which kills the process if it gets called while
/// `IN_FORKED_CHILD` is set.
struct TrapAllocator;

impl TrapAllocator {
    fn check(&self) {
        if IN_FORKED_CHILD.load(Ordering::SeqCst) {
            const MSG: &[u8] = b"the allocator was called in the forked child\n";
            // Safety: `write` and `_exit` are both async-signal-safe, which is
            // the whole point. We can't use `panic!` or `eprintln!` here since
            // they would allocate and land us right back in this routine.
            unsafe {
                libc::write(libc::STDERR_FILENO, MSG.as_ptr().cast(), MSG.len());
                libc::_exit(ALLOCATED_EXIT_CODE);
            }
        }
    }
}

// Safety: every routine just forwards to the system allocator, which is a
// valid implementation of the trait.
unsafe impl GlobalAlloc for TrapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.check();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.check();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.check();
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        self.check();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// A `pthread_atfork` child handler, so that the flag gets set the instant the
/// child comes into existence.
extern "C" fn mark_forked_child() {
    IN_FORKED_CHILD.store(true, Ordering::SeqCst);
}

#[test]
fn it_does_not_allocate_in_the_child() {
    // Safety: basic ffi, and the handler we register only touches an atomic.
    assert_eq!(unsafe { libc::pthread_atfork(None, None, Some(mark_forked_child)) }, 0);

    let fork = Fork::from_ptmx();

    // Nothing above this line may allocate in the child. Both halves of the
    // fork land here, but the flag only got set in the child.
    let is_child = IN_FORKED_CHILD.swap(false, Ordering::SeqCst);
    if is_child {
        // We made it through `Fork::new` without tripping the allocator.
        let code = if fork.is_ok() { 0 } else { FORK_FAILED_EXIT_CODE };
        // Safety: basic ffi. We can't return normally or the test harness
        // would run the rest of the suite a second time in the child.
        unsafe { libc::_exit(code) };
    }

    let fork = fork.expect("the fork should have succeeded");
    let pid = fork.child_pid().expect("the parent should know the child's pid");

    let mut status = 0;
    // Safety: basic ffi, the pid is valid since `fork` is still alive.
    let waited = unsafe { libc::waitpid(pid, &mut status, 0) };
    assert_eq!(waited, pid, "waitpid should reap our child");
    assert!(libc::WIFEXITED(status), "the child should have exited normally");
    match libc::WEXITSTATUS(status) {
        0 => (),
        ALLOCATED_EXIT_CODE => {
            panic!("the child allocated between fork() and Fork::new() returning")
        }
        FORK_FAILED_EXIT_CODE => panic!("the child half of the fork failed"),
        code => panic!("the child exited with an unexpected status: {code}"),
    }
}
