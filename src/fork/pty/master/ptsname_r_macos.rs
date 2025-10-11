//! macOS/iOS implementation of ptsname_r
//!
//! As `ptsname_r()` is not available on macOS/iOS, this provides a compatible
//! implementation using the `TIOCPTYGNAME` ioctl syscall.
//!
//! Based on: https://tarq.net/posts/ptsname-on-osx-with-rust/

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[inline]
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

    if libc::ioctl(fd, libc::TIOCPTYGNAME as u64, &mut ioctl_buf) != 0 {
        return *libc::__error();
    }

    let mut len = 0;
    while len < IOCTL_BUF_SIZE && ioctl_buf[len] != 0 {
        len += 1;
    }
    len += 1;

    if len > buflen {
        return libc::ERANGE;
    }

    libc::memcpy(
        buf as *mut libc::c_void,
        ioctl_buf.as_ptr() as *const libc::c_void,
        len,
    );

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn test_ptsname_r_retrieves_valid_name() {
        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
        assert!(master_fd >= 0, "Failed to open master PTY");

        unsafe {
            assert_eq!(libc::grantpt(master_fd), 0, "grantpt failed");
            assert_eq!(libc::unlockpt(master_fd), 0, "unlockpt failed");
        }

        let mut buf = vec![0u8; 1024];
        let result =
            unsafe { ptsname_r(master_fd, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };

        unsafe {
            libc::close(master_fd);
        }

        assert_eq!(result, 0, "ptsname_r failed with error code: {}", result);

        let name = unsafe { CStr::from_ptr(buf.as_ptr() as *const libc::c_char) };
        let name_str = name.to_str().expect("Invalid UTF-8 in PTY name");

        assert!(
            name_str.starts_with("/dev/ttys") || name_str.starts_with("/dev/pty"),
            "Unexpected PTY name format: {}",
            name_str
        );
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn test_ptsname_r_buffer_too_small() {
        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };

        if master_fd >= 0 {
            unsafe {
                libc::grantpt(master_fd);
                libc::unlockpt(master_fd);
            }

            let mut buf = [0u8; 2];
            let result =
                unsafe { ptsname_r(master_fd, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };

            unsafe {
                libc::close(master_fd);
            }

            assert_eq!(result, libc::ERANGE);
        }
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn test_ptsname_r_invalid_fd() {
        let mut buf = vec![0u8; 1024];
        let result = unsafe { ptsname_r(-1, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };

        assert_ne!(result, 0, "Expected non-zero error code for invalid fd");
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn test_ptsname_r_null_buffer() {
        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };

        if master_fd >= 0 {
            unsafe {
                libc::grantpt(master_fd);
                libc::unlockpt(master_fd);
            }

            let result = unsafe { ptsname_r(master_fd, std::ptr::null_mut(), 1024) };

            unsafe {
                libc::close(master_fd);
            }

            assert_eq!(result, libc::EINVAL);
        }
    }
}
