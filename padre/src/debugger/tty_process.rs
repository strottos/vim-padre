use std::cell::RefCell;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::process::exit;

use bytes::{BufMut, Bytes, BytesMut};
use nix::fcntl::{open, OFlag};
use nix::libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use nix::pty::{grantpt, posix_openpt, unlockpt, PtyMaster};
use nix::sys::stat;
use nix::unistd::{dup, dup2, execvp, fork, setsid, ForkResult};
use std::os::unix::io::{AsRawFd, FromRawFd};
use tokio::prelude::*;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_reactor::PollEvented;

// Code based on https://github.com/philippkeller/rexpect/blob/master/src/process.rs
#[cfg(target_os = "linux")]
use nix::pty::ptsname_r;

#[cfg(target_os = "macos")]
/// ptsname_r is a linux extension but ptsname isn't thread-safe
/// instead of using a static mutex this calls ioctl with TIOCPTYGNAME directly
/// based on https://blog.tarq.io/ptsname-on-osx-with-rust/
fn ptsname_r(fd: &PtyMaster) -> nix::Result<String> {
    use nix::libc::{ioctl, TIOCPTYGNAME};
    use std::ffi::CStr;

    // the buffer size on OSX is 128, defined by sys/ttycom.h
    let buf: [i8; 128] = [0; 128];

    unsafe {
        match ioctl(fd.as_raw_fd(), TIOCPTYGNAME as u64, &buf) {
            0 => {
                let res = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
                Ok(res)
            }
            _ => Err(nix::Error::last()),
        }
    }
}

#[derive(Debug)]
pub struct TtyFile {
    inner: File,
    fd: i32,
    evented: RefCell<Option<mio::Registration>>,
}

impl TtyFile {
    pub fn new(pty_master: PtyMaster) -> Self {
        let fd = dup(pty_master.as_raw_fd()).unwrap();

        // Set non blocking
        unsafe {
            let previous = libc::fcntl(fd, libc::F_GETFL);
            let new = previous | libc::O_NONBLOCK;
            if new != previous {
                libc::fcntl(fd, libc::F_SETFL, new);
            }
        }

        let inner = unsafe { File::from_raw_fd(fd) };

        TtyFile {
            inner: inner,
            fd: fd,
            evented: Default::default(),
        }
    }

    pub fn into_io(self) -> io::Result<PollEvented<Self>> {
        PollEvented::new_with_handle(self, &tokio::reactor::Handle::default())
    }
}

impl mio::Evented for TtyFile {
    fn register(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> io::Result<()> {
        mio::unix::EventedFd(&self.fd).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> io::Result<()> {
        match &*self.evented.borrow() {
            &None => mio::unix::EventedFd(&self.fd).reregister(poll, token, interest, opts),
            &Some(ref r) => r.reregister(poll, token, interest, opts),
        }
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        match &*self.evented.borrow() {
            &None => mio::unix::EventedFd(&self.fd).deregister(poll),
            &Some(ref r) => mio::Evented::deregister(r, poll),
        }
    }
}

impl Read for TtyFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for TtyFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub struct TtyFileStdioStream {
    io: PollEvented<TtyFile>,
    stdin_rx: Receiver<Bytes>,
    bytes_mut: BytesMut,
}

impl TtyFileStdioStream {
    pub fn new(tty: TtyFile, stdin_rx: Receiver<Bytes>) -> Self {
        TtyFileStdioStream {
            io: tty.into_io().expect("Unable to read TTY"),
            stdin_rx,
            bytes_mut: BytesMut::new(),
        }
    }
}

// TODO: May not work correctly having stdin and stdout in one stream like this, but let's see what
// happens... Might be OK as we don't tend to do stdin while stdout is happening, could be wrong.
impl Stream for TtyFileStdioStream {
    type Item = Bytes;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.stdin_rx.poll().unwrap() {
            Async::Ready(Some(v)) => {
                match self.io.poll_write_ready() {
                    Ok(Async::Ready(_)) => {
                        match self.io.write(Bytes::from(&v[..]).as_ref()) {
                            Ok(_) => (),
                            Err(err) => {
                                if err.kind() == io::ErrorKind::WouldBlock {
                                    return Ok(Async::NotReady);
                                }
                                return Err(err);
                            }
                        };
                    }
                    Ok(Async::NotReady) => {
                        println!("Not ready write");
                        return Ok(Async::NotReady);
                    }
                    Err(err) => {
                        return Err(err);
                    }
                };
            }
            _ => {}
        }

        loop {
            match self.io.poll_read_ready(mio::Ready::readable()) {
                Ok(Async::Ready(_)) => {
                    let mut buffer: [u8; 512] = [0; 512];
                    match self.io.read(&mut buffer) {
                        Ok(_) => {
                            // TODO: More efficient, this is crap, but works for now
                            for byte in buffer.iter() {
                                if *byte != 0 {
                                    self.bytes_mut.reserve(1);
                                    self.bytes_mut.put(*byte);
                                }
                            }
                        }
                        Err(err) => {
                            if err.kind() == io::ErrorKind::WouldBlock {
                                if self.bytes_mut.len() > 0 {
                                    let bytes = self.bytes_mut.clone().freeze();
                                    self.bytes_mut = BytesMut::new();
                                    return Ok(Async::Ready(Some(bytes)));
                                }
                                return Ok(Async::NotReady);
                            }
                            return Err(err);
                        }
                    }
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(err) => return Err(err),
            }
        }
    }
}

pub fn spawn_process(argv: Vec<String>, stdin_rx: Receiver<Bytes>, stdout_tx: Sender<Bytes>) {
    // Code based on https://github.com/philippkeller/rexpect/blob/master/src/process.rs
    let master_fd = posix_openpt(OFlag::O_RDWR).unwrap();

    // Allow a slave to be generated for it
    grantpt(&master_fd).unwrap();
    unlockpt(&master_fd).unwrap();

    // on Linux this is the libc function, on OSX this is our implementation of ptsname_r
    let slave_name = ptsname_r(&master_fd).unwrap();

    println!("Spawning {:?}", argv);

    match fork().unwrap() {
        ForkResult::Child => {
            setsid().unwrap(); // create new session with child as session leader
            let slave_fd = open(
                std::path::Path::new(&slave_name),
                OFlag::O_RDWR,
                stat::Mode::empty(),
            )
            .unwrap();

            // assign stdin, stdout, stderr to the tty, just like a terminal does
            dup2(slave_fd, STDIN_FILENO).unwrap();
            dup2(slave_fd, STDOUT_FILENO).unwrap();
            dup2(slave_fd, STDERR_FILENO).unwrap();

            // set echo off?
            //let mut flags = termios::tcgetattr(STDIN_FILENO).unwrap();
            //flags.local_flags &= !termios::LocalFlags::ECHO;
            //termios::tcsetattr(STDIN_FILENO, termios::SetArg::TCSANOW, &flags).unwrap();

            let argv: Vec<CString> = argv
                .iter()
                .map(|x| CString::new(x.clone()).unwrap())
                .collect();
            let path = argv[0].clone();

            execvp(&path, &argv[..]).unwrap();

            exit(-1);
        }

        ForkResult::Parent { child: _child_pid } => {
            let tty = TtyFile::new(master_fd);

            let mut out = io::stdout();

            tokio::spawn(
                TtyFileStdioStream::new(tty, stdin_rx)
                    .for_each(move |chunk| {
                        out.write_all(&chunk).unwrap();
                        tokio::spawn(
                            stdout_tx
                                .clone()
                                .send(chunk)
                                .map(|_| {})
                                .map_err(|e| {
                                    // TODO: Error handling?
                                    println!("Can't send output to be analysed: {}", e)
                                })
                        );
                        out.flush()
                    })
                    .map_err(|e| println!("error reading stdout; error = {:?}", e)),
            );
        }
    };
}

//TODO:
//#[cfg(test)]
//mod tests {
//    use tokio::sync::mpsc;
//
//    #[test]
//    fn check_spawn_communicate_process() {
//        let mut runtime = Runtime::new().unwrap();
//        let (tx, rx) = mpsc::channel(32);
//        // TODO: Lazy future?
//        super::spawn_process(vec!("node".to_string(), "test_files/echo_stdin.js".to_string()), rx);
//        tokio::run(fut)
//    }
//}
