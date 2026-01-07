extern crate libc;
extern crate shpool_pty;

use self::shpool_pty::prelude::*;

use std::io::prelude::*;
use std::process::Command;

fn read_line(master: &mut Master) -> String {
    let mut buf = [0];
    let mut res = String::new();
    while buf[0] as char != '\n' {
        master.read_exact(&mut buf).expect("cannot read 1 byte");
        res.push(buf[0] as char)
    }
    res
}

#[test]
fn it_can_read_write() {
    let fork = Fork::from_ptmx().unwrap();

    if let Ok(mut master) = fork.is_parent() {
        let _ = master.write("echo readme!\n".to_string().as_bytes());

        read_line(&mut master); // this is the "echo readme!" we just sent
        read_line(&mut master); // this is the shell and "echo readme!" again
        assert_eq!(read_line(&mut master).trim(), "readme!");
        let _ = master.write("exit\n".to_string().as_bytes());
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("--rcfile").arg("/dev/null");
        cmd.env_clear();

        cmd.env("PS1", "");

        // On macOS, silence the deprecation warning;
        // https://github.com/apple-oss-distributions/bash/blob/e86b2aa8e37a31f8fce56366d1abaf08a3fac7d2/bash-3.2/shell.c#L760-L765
        #[cfg(target_os = "macos")]
        {
            cmd.env("BASH_SILENCE_DEPRECATION_WARNING", "1");
        }

        let _ = cmd.status();
    }
}
