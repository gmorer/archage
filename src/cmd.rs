use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::process::{Command, ExitStatus, Stdio};

use nix::sys::epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags};

// Kindof like combined output of go/exec
pub fn command(mut cmd: Command) -> Result<(ExitStatus, Vec<String>), ()> {
    let mut output = Vec::new();
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let poll = Epoll::new(EpollCreateFlags::empty()).unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_fd = stdout.as_raw_fd();
    let stderr_fd = stderr.as_raw_fd();
    nix::ioctl_write_int!(stdout_fd, 0x5421 /* FIONBIO */, 1);
    nix::ioctl_write_int!(stderr_fd, 0x5421 /* FIONBIO */, 1);
    let flags = EpollFlags::EPOLLIN /* read */ | EpollFlags::EPOLLHUP /* close */;
    poll.add(stdout.as_fd(), EpollEvent::new(flags, 0)).unwrap();
    poll.add(stderr.as_fd(), EpollEvent::new(flags, 1)).unwrap();
    let mut stdout_buffer = String::new();
    let mut stderr_buffer = String::new();
    let mut buff = [0; 500];
    let status = loop {
        let mut events = [EpollEvent::empty(), EpollEvent::empty()];
        match poll.wait(&mut events, 100) {
            Ok(0) => {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
            }
            Ok(x) => {
                if events[0].events().contains(EpollFlags::EPOLLHUP) {
                    if let Some(status) = child.try_wait().unwrap() {
                        break status;
                    }
                }
                for ev in 0..x {
                    let (fd, raw_fd, line_buffer) = if events[ev].data() == 0 {
                        (stdout.as_fd(), stdout_fd, &mut stdout_buffer)
                    } else if events[ev].data() == 1 {
                        (stderr.as_fd(), stderr_fd, &mut stderr_buffer)
                    } else {
                        eprintln!("Should no tbe possible");
                        continue;
                    };
                    if events[ev].events().contains(EpollFlags::EPOLLHUP) {
                        poll.delete(fd).unwrap();
                        continue;
                    }
                    match nix::unistd::read(raw_fd, &mut buff) {
                        Ok(n) => {
                            line_buffer.push_str(&String::from_utf8_lossy(&buff[..n]));
                        }
                        Err(e) => {
                            println!("error while readig output: {}", e);
                        }
                    }
                    let mut tmp_line = "";
                    for line in line_buffer.split('\n') {
                        tmp_line = line;
                        if !line.is_empty() {
                            println!("{}", line);
                            output.push(line.to_string());
                        }
                    }
                    *line_buffer = tmp_line.to_string();
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                panic!("error");
            }
        }
    };
    Ok((status, output))
}
