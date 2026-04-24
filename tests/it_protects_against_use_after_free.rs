extern crate libc;
extern crate shpool_pty;

use self::shpool_pty::prelude::*;

#[test]
fn it_drops_correctly() {
    let fork = Fork::from_ptmx().expect("failed to fork");

    let master = match fork.is_parent() {
        Ok(m) => m,
        Err(_) => {
            // Child process
            unsafe { libc::_exit(0) };
        }
    };

    let fd = master.raw_fd();

    // Check if fd is valid
    assert!(
        unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1,
        "FD should be valid"
    );

    let master_clone = master.clone();
    assert_eq!(master.raw_fd(), master_clone.raw_fd());

    drop(master);
    // FD should still be valid because master_clone exists
    assert!(
        unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1,
        "FD should still be valid after first drop"
    );

    drop(master_clone);
    // FD should still be valid because fork still exists and holds a master
    assert!(
        unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1,
        "FD should still be valid because fork exists"
    );

    drop(fork);
    // Now it should be closed
    assert!(
        unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1,
        "FD should be closed after last drop"
    );
}

#[test]
fn fork_drop_does_not_close_master() {
    let fork = Fork::from_ptmx().expect("failed to fork");

    let master = match fork.is_parent() {
        Ok(m) => m,
        Err(_) => {
            // Child process
            unsafe { libc::_exit(0) };
        }
    };

    let fd = master.raw_fd();

    drop(fork);
    // Master should still be valid
    assert!(
        unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1,
        "FD should still be valid after fork drop"
    );

    drop(master);
    // Now it should be closed
    assert!(
        unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1,
        "FD should be closed after master drop"
    );
}

#[test]
fn slave_cloning_and_dropping() {
    let master =
        Master::new(&std::ffi::CString::new("/dev/ptmx").unwrap()).expect("failed to open ptmx");
    master.grantpt().expect("failed to grantpt");
    master.unlockpt().expect("failed to unlockpt");

    let mut buf = [0u8; 128];
    master.ptsname_r(&mut buf).expect("failed to get ptsname");
    let name = std::ffi::CStr::from_bytes_until_nul(&buf).expect("failed to parse ptsname");

    let slave = Slave::new(name).expect("failed to open slave");
    let fd = slave.raw_fd();

    let slave_clone = slave.clone();
    assert_eq!(slave.raw_fd(), slave_clone.raw_fd());

    drop(slave);
    assert!(
        unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1,
        "Slave FD should still be valid after first drop"
    );

    drop(slave_clone);
    assert!(
        unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1,
        "Slave FD should be closed after last drop"
    );
}
