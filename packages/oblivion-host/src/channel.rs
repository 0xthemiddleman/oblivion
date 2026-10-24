use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

/// One deadline for a complete exchange, including fragmented reads and writes.
/// A new exchange must construct a new Channel, never derive its budget from
/// the socket timeout left by the previous operation.
pub struct Channel<'a> {
    stream: &'a UnixStream,
    deadline: Instant,
}

impl<'a> Channel<'a> {
    pub fn new(stream: &'a UnixStream, budget: Duration) -> Self {
        Self {
            stream,
            deadline: Instant::now() + budget,
        }
    }

    pub fn remaining(&self) -> io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))
    }

    // Also used around the single-byte SCM_RIGHTS transfer. Its following
    // metadata response shares this Channel's deadline.
    pub fn prepare_read(&self) -> io::Result<()> {
        self.stream.set_read_timeout(Some(self.remaining()?))
    }

    pub fn prepare_write(&self) -> io::Result<()> {
        self.stream.set_write_timeout(Some(self.remaining()?))
    }
}

impl Read for Channel<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            self.prepare_read()?;
            let result = self.stream.read(bytes);
            self.remaining()?;
            match result {
                Err(error) if retry(&error) => continue,
                result => return result,
            }
        }
    }
}

impl Write for Channel<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            self.prepare_write()?;
            let result = self.stream.write(bytes);
            self.remaining()?;
            match result {
                Err(error) if retry(&error) => continue,
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.remaining().map(|_| ())
    }
}

fn retry(error: &io::Error) -> bool {
    // Socket timeout resolution differs across kernels. Retry an early kernel
    // timeout against the same monotonic deadline, never a fresh budget.
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}
