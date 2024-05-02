use log::{debug, error};
use nix::sys::epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::os::fd::{AsFd, AsRawFd};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecError {
    #[error("System error: {0}")]
    Io(#[from] IoError),
    #[error("System error: Erno: {0}")]
    Errno(#[from] nix::errno::Errno),
}

#[derive(Debug)]
pub struct CmdError {
    pub e: Vec<String>,
}

impl std::fmt::Display for CmdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.e)
    }
}

impl std::error::Error for CmdError {}

impl CmdError {
    pub fn from_output(out: Vec<String>) -> Self {
        Self { e: out }
    }
}

// Write last lines from an outputs to the logs
pub fn write_last_lines(lines: &[String], n: u32) {
    let length = lines.len() as u32;
    for i in 0..n {
        let i = n - i;
        if i > length {
            continue;
        }
        if let Some(line) = lines.get((length - i) as usize) {
            error!("---- {}", line);
        }
    }
}

// Kindof like combined output of go/exec
fn _command(mut cmd: Command) -> Result<(ExitStatus, Vec<String>), ExecError> {
    let mut output = Vec::new();
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let poll = Epoll::new(EpollCreateFlags::empty())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| IoError::new(IoErrorKind::BrokenPipe, "No stdout on spawn child"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| IoError::new(IoErrorKind::BrokenPipe, "No stderr on spawn child"))?;
    let stdout_fd = stdout.as_raw_fd();
    let stderr_fd = stderr.as_raw_fd();
    nix::ioctl_write_int!(stdout_fd, 0x5421 /* FIONBIO */, 1);
    nix::ioctl_write_int!(stderr_fd, 0x5421 /* FIONBIO */, 1);
    let flags = EpollFlags::EPOLLIN /* read */ | EpollFlags::EPOLLHUP /* close */;
    poll.add(stdout.as_fd(), EpollEvent::new(flags, 0))?;
    poll.add(stderr.as_fd(), EpollEvent::new(flags, 1))?;
    let mut stdout_buffer = String::new();
    let mut stderr_buffer = String::new();
    let mut buff = [0; 500];
    let status = loop {
        let mut events = [EpollEvent::empty(), EpollEvent::empty()];
        let x = poll.wait(&mut events, 100)?;
        if let Some(status) = match child.try_wait() {
            Ok(res) => res,
            Err(e) => {
                error!("Error while waiting for child process: {}", e);
                continue;
            }
        } {
            break status;
        }
        for ev in 0..x {
            let (fd, raw_fd, line_buffer) = if events[ev].data() == 0 {
                (stdout.as_fd(), stdout_fd, &mut stdout_buffer)
            } else if events[ev].data() == 1 {
                (stderr.as_fd(), stderr_fd, &mut stderr_buffer)
            } else {
                error!("Should not be possible");
                continue;
            };
            if events[ev].events().contains(EpollFlags::EPOLLHUP) {
                // TODO: Error but with the output
                poll.delete(fd)?;
                continue;
            }
            match nix::unistd::read(raw_fd, &mut buff) {
                Ok(n) => {
                    line_buffer.push_str(&String::from_utf8_lossy(&buff[..n]));
                }
                Err(e) => {
                    error!("error while readig output: {}", e);
                }
            }
            let mut offset = 0;
            while let Some(index) = line_buffer[offset..].find('\n') {
                let line = &line_buffer[offset..(index + offset)];
                debug!("{}", line);
                output.push(line.to_string());
                offset += index + 1;
            }
            *line_buffer = line_buffer[offset..].to_string();
        }
    };
    Ok((status, output))
}

pub fn command<P>(args: &[&str], current_dir: P) -> Result<(ExitStatus, Vec<String>), ExecError>
where
    P: AsRef<Path>,
{
    assert!(args.len() > 0);
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..]);
    cmd.current_dir(current_dir);
    _command(cmd)
}
