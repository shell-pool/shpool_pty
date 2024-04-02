extern crate errno;
extern crate libc;
extern crate shpool_pty;

use shpool_pty::fork::*;
use std::io::Read;
use std::process::Command;

fn main() {
    let fork = Fork::from_ptmx().unwrap();

    if let Ok(mut master) = fork.is_parent() {
        // Read output via PTY master
        let mut output = String::new();

        match master.read_to_string(&mut output) {
            Ok(_nread) => println!("child tty is: {}", output.trim()),
            Err(e) => panic!("read error: {}", e),
        }
    } else {
        // Child process just exec `tty`
        Command::new("tty").status().expect("could not execute tty");
    }
}
