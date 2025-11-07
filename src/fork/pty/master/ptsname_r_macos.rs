//! macOS implementation of ptsname_r
//!
//! As `ptsname_r()` is not available on macOS, this provides a compatible
//! implementation using the `TIOCPTYGNAME` ioctl syscall.
//!
//! Based on: https://tarq.net/posts/ptsname-on-osx-with-rust/

#[cfg(target_os = "macos")]
/// # Safety
///
/// Callers must uphold the following invariants:
/// - `fd` must refer to an open master PTY file descriptor.
/// - `buf` must point to a valid allocation with capacity of at least `buflen` bytes and the
///   allocation must remain valid throughout the function call.
pub unsafe fn ptsname_r(
    fd: libc::c_int,
    buf: *mut libc::c_char,
    buflen: libc::size_t,
) -> libc::c_int {
    const IOCTL_BUF_SIZE: usize = 128;

    if buf.is_null() || buflen == 0 {
        return libc::EINVAL;
    }

    let mut ioctl_buf: [libc::c_char; IOCTL_BUF_SIZE] = [0; IOCTL_BUF_SIZE];

    if libc::ioctl(fd, libc::TIOCPTYGNAME as libc::c_ulong, &mut ioctl_buf) != 0 {
        return *libc::__error();
    }

    let null_ptr = libc::memchr(ioctl_buf.as_ptr() as *const libc::c_void, 0, IOCTL_BUF_SIZE);

    let len = if null_ptr.is_null() {
        IOCTL_BUF_SIZE
    } else {
        let offset = (null_ptr as usize) - (ioctl_buf.as_ptr() as usize);
        offset + 1 // include null terminator.
    };

    if len > buflen {
        return libc::ERANGE;
    }

    std::ptr::copy(ioctl_buf.as_ptr(), buf, len);

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[cfg(target_os = "macos")]
    #[test]
    fn test_ptsname_r_retrieves_valid_name() {
        // Safety: calling libc function with valid flags.
        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
        assert!(master_fd >= 0, "Failed to open master PTY");

        // Safety: master_fd is a valid open file descriptor.
        unsafe {
            assert_eq!(libc::grantpt(master_fd), 0, "grantpt failed");
            assert_eq!(libc::unlockpt(master_fd), 0, "unlockpt failed");
        }

        let mut buf = vec![0u8; 1024];
        // Safety: master_fd is valid, buf is a properly sized allocation.
        let result =
            unsafe { ptsname_r(master_fd, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };

        // Safety: closing the fd opened above.
        unsafe {
            libc::close(master_fd);
        }

        assert_eq!(result, 0, "ptsname_r failed with error code: {}", result);

        // Safety: buf contains a null-terminated string from ptsname_r.
        let name = unsafe { CStr::from_ptr(buf.as_ptr() as *const libc::c_char) };
        let name_str = name.to_str().expect("Invalid UTF-8 in PTY name");

        assert!(
            name_str.starts_with("/dev/ttys") || name_str.starts_with("/dev/pty"),
            "Unexpected PTY name format: {}",
            name_str
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_ptsname_r_buffer_too_small() {
        // Safety: calling libc function with valid flags.
        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };

        if master_fd >= 0 {
            // Safety: master_fd is a valid open file descriptor.
            unsafe {
                libc::grantpt(master_fd);
                libc::unlockpt(master_fd);
            }

            let mut buf = [0u8; 2];
            // Safety: master_fd is valid, buf is a properly sized allocation though intentionally
            // too small.
            let result =
                unsafe { ptsname_r(master_fd, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };

            // Safety: closing the fd opened above.
            unsafe {
                libc::close(master_fd);
            }

            assert_eq!(result, libc::ERANGE);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_ptsname_r_invalid_fd() {
        let mut buf = vec![0u8; 1024];
        // Safety: buf is a properly sized allocation.
        let result = unsafe { ptsname_r(-1, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };

        assert_ne!(result, 0, "Expected non-zero error code for invalid fd");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_ptsname_r_null_buffer() {
        // Safety: calling libc function with valid flags.
        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };

        if master_fd >= 0 {
            // Safety: master_fd is a valid open file descriptor.
            unsafe {
                libc::grantpt(master_fd);
                libc::unlockpt(master_fd);
            }

            // Safety: function handles null buffer safely.
            let result = unsafe { ptsname_r(master_fd, std::ptr::null_mut(), 1024) };

            // Safety: closing the fd opened above.
            unsafe {
                libc::close(master_fd);
            }

            assert_eq!(result, libc::EINVAL);
        }
    }
}
