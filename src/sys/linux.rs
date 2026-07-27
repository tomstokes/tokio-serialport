use crate::settings::Settings;
use crate::{DataBits, FlowControl, Parity, StopBits};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub(crate) struct Port {
    fd: AsyncFd<OwnedFd>,
}

impl Port {
    pub(crate) async fn open(path: PathBuf, settings: Settings) -> std::io::Result<Self> {
        let fd = match tokio::task::spawn_blocking(move || open_port(&path, settings)).await {
            Ok(result) => result?,
            // `tokio::task::spawn_blocking` catches panics. Re-panic if that happens.
            Err(error) if error.is_panic() => {
                std::panic::resume_unwind(error.into_panic());
            }
            Err(error) => {
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, error));
            }
        };
        let async_fd = AsyncFd::new(fd)?;
        Ok(Self { fd: async_fd })
    }

    pub(crate) fn set_dtr(&self, asserted: bool) -> std::io::Result<()> {
        set_modem_line(self.as_fd(), libc::TIOCM_DTR, asserted)
    }

    pub(crate) fn set_rts(&self, asserted: bool) -> std::io::Result<()> {
        set_modem_line(self.as_fd(), libc::TIOCM_RTS, asserted)
    }

    pub(crate) fn cts(&self) -> std::io::Result<bool> {
        read_modem_line(self.as_fd(), libc::TIOCM_CTS)
    }

    pub(crate) fn dsr(&self) -> std::io::Result<bool> {
        read_modem_line(self.as_fd(), libc::TIOCM_DSR)
    }

    pub(crate) fn ring_indicator(&self) -> std::io::Result<bool> {
        read_modem_line(self.as_fd(), libc::TIOCM_RI)
    }

    pub(crate) fn carrier_detect(&self) -> std::io::Result<bool> {
        read_modem_line(self.as_fd(), libc::TIOCM_CD)
    }
}

impl AsyncRead for Port {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let this = self.get_mut();
        loop {
            let mut guard = ready!(this.fd.poll_read_ready(cx))?;
            let destination = buf.initialize_unfilled();
            match guard.try_io(|async_fd| read_fd(async_fd.as_fd(), destination)) {
                Ok(Ok(count)) => {
                    buf.advance(count);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(error)) => return Poll::Ready(Err(error)),
                Err(_would_block) => continue,
            }
        }
    }
}

impl AsyncWrite for Port {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let this = self.get_mut();
        loop {
            let mut guard = ready!(this.fd.poll_write_ready(cx))?;
            match guard.try_io(|async_fd| write_fd(async_fd.as_fd(), buf)) {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        // This does not call `tcdrain` because it can block, possibly indefinitely if flow control
        // is enabled and the other side does not acknowledge.
        // TODO: Document what this means
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl AsFd for Port {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl AsRawFd for Port {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

fn set_modem_line(fd: BorrowedFd<'_>, line: libc::c_int, asserted: bool) -> std::io::Result<()> {
    let request = if asserted {
        libc::TIOCMBIS
    } else {
        libc::TIOCMBIC
    };
    syscall_result(unsafe { libc::ioctl(fd.as_raw_fd(), request, &line) })
}

fn read_modem_line(fd: BorrowedFd<'_>, line: libc::c_int) -> std::io::Result<bool> {
    let mut status = 0;
    syscall_result(unsafe { libc::ioctl(fd.as_raw_fd(), libc::TIOCMGET, &mut status) })?;
    Ok(status & line != 0)
}

/// Helper to read from a `BorrowedFd` and retry syscall if interrupted
fn read_fd(fd: BorrowedFd<'_>, buffer: &mut [u8]) -> std::io::Result<usize> {
    loop {
        let result =
            unsafe { libc::read(fd.as_raw_fd(), buffer.as_mut_ptr().cast(), buffer.len()) };
        if result >= 0 {
            return Ok(result as usize);
        }

        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        return Err(error);
    }
}

/// Helper to write to a `BorrowedFd` and retry syscall if interrupted
fn write_fd(fd: BorrowedFd<'_>, buffer: &[u8]) -> std::io::Result<usize> {
    loop {
        let result = unsafe { libc::write(fd.as_raw_fd(), buffer.as_ptr().cast(), buffer.len()) };
        if result >= 0 {
            return Ok(result as usize);
        }

        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        return Err(error);
    }
}

/// Synchronously open and configure a serial port at the given path.
///
/// Opening and configuring a serial port can block and must be called through
/// `tokio::task::spawn_blocking`.
fn open_port(path: &Path, settings: Settings) -> std::io::Result<OwnedFd> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY | libc::O_CLOEXEC)
        .open(path)?;
    let fd = OwnedFd::from(file);

    set_exclusive(fd.as_fd())?;

    configure_port(fd.as_fd(), &settings)?;

    Ok(fd)
}

fn set_exclusive(fd: BorrowedFd<'_>) -> std::io::Result<()> {
    syscall_result(unsafe { libc::ioctl(fd.as_raw_fd(), libc::TIOCEXCL) })
}

fn configure_port(fd: BorrowedFd<'_>, settings: &Settings) -> std::io::Result<()> {
    let raw_fd = fd.as_raw_fd();

    let mut termios = MaybeUninit::<libc::termios>::uninit();
    syscall_result(unsafe { libc::tcgetattr(raw_fd, termios.as_mut_ptr()) })?;
    let mut termios = unsafe { termios.assume_init() };

    apply_settings(&mut termios, settings)?;

    syscall_result(unsafe { libc::tcsetattr(raw_fd, libc::TCSANOW, &termios) })
}

fn apply_settings(termios: &mut libc::termios, settings: &Settings) -> std::io::Result<()> {
    // Configure the tty to operate in raw mode
    unsafe {
        libc::cfmakeraw(termios);
    }

    // Enable receiving
    termios.c_cflag |= libc::CREAD;

    // Ignore modem control lines
    termios.c_cflag |= libc::CLOCAL;

    // Configure VMIN and VTIME baseline as expected for our O_NONBLOCK configuration
    termios.c_cc[libc::VMIN] = 1;
    termios.c_cc[libc::VTIME] = 0;

    // Set data bits character size mask
    termios.c_cflag &= !libc::CSIZE;
    termios.c_cflag |= match settings.data_bits {
        DataBits::Five => libc::CS5,
        DataBits::Six => libc::CS6,
        DataBits::Seven => libc::CS7,
        DataBits::Eight => libc::CS8,
    };

    // Configure parity
    termios.c_cflag &= !(libc::PARENB | libc::PARODD);
    termios.c_iflag &= !(libc::INPCK | libc::IGNPAR | libc::PARMRK);
    match settings.parity {
        Parity::None => {}
        Parity::Odd => {
            // Enable parity generation on output and parity checking on input
            // Use odd parity
            termios.c_cflag |= libc::PARENB | libc::PARODD;
            // Enable input parity checking
            termios.c_iflag |= libc::INPCK;
        }
        Parity::Even => {
            // Enable parity generation on output and parity checking on input
            termios.c_cflag |= libc::PARENB;
            // Enable input parity checking
            termios.c_iflag |= libc::INPCK;
        }
    }

    // Set number of stop bits (`CSTOPB` enabled for 2 stop bits, disabled for 1)
    match settings.stop_bits {
        StopBits::One => termios.c_cflag &= !libc::CSTOPB,
        StopBits::Two => termios.c_cflag |= libc::CSTOPB,
    }

    // Clear flow control flags
    termios.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);
    termios.c_cflag &= !libc::CRTSCTS;

    // Configure flow control
    match settings.flow_control {
        FlowControl::None => {}
        FlowControl::Software => {
            // Enable XON/XOFF flow control on output
            // Enable XON/XOFF flow control on input
            termios.c_iflag |= libc::IXON | libc::IXOFF;

            // Conventional XON/XOFF START character
            termios.c_cc[libc::VSTART] = 0x11;

            // Conventional XON/XOFF STOP character
            termios.c_cc[libc::VSTOP] = 0x13;
        }
        FlowControl::Hardware => {
            // CTS output flow control and RTS input flow control
            termios.c_cflag |= libc::CRTSCTS;
        }
    }

    // Configure baud rate
    // TODO: Support arbitrary baud rates
    let baud_rate = standard_baud_rate(settings.baud_rate)
        .ok_or_else(|| {
            todo!("Custom baud rates not yet implemented");
        })
        .unwrap();
    syscall_result(unsafe { libc::cfsetspeed(termios, baud_rate) })
}

fn standard_baud_rate(baud_rate: u32) -> Option<libc::speed_t> {
    // The termios manual page lists four of the baud rates as SPARC only, with another four
    // available only on non-SPARC platforms. Most modern platforms probably support custom baud
    // rate settings well enough that this is moot, but implementing this accurately is worth the
    // minimal extra lines of code.
    // REF: https://man7.org/linux/man-pages/man3/termios.3.html
    Some(match baud_rate {
        50 => libc::B50,
        75 => libc::B75,
        110 => libc::B110,
        134 => libc::B134,
        150 => libc::B150,
        200 => libc::B200,
        300 => libc::B300,
        600 => libc::B600,
        1_200 => libc::B1200,
        1_800 => libc::B1800,
        2_400 => libc::B2400,
        4_800 => libc::B4800,
        9_600 => libc::B9600,
        19_200 => libc::B19200,
        38_400 => libc::B38400,
        57_600 => libc::B57600,
        115_200 => libc::B115200,
        230_400 => libc::B230400,
        460_800 => libc::B460800,
        500_000 => libc::B500000,
        576_000 => libc::B576000,
        921_600 => libc::B921600,
        1_000_000 => libc::B1000000,
        1_152_000 => libc::B1152000,
        1_500_000 => libc::B1500000,
        2_000_000 => libc::B2000000,
        #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
        76_800 => libc::B76800,
        #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
        153_600 => libc::B153600,
        #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
        307_200 => libc::B307200,
        #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
        614_400 => libc::B614400,
        #[cfg(not(any(target_arch = "sparc", target_arch = "sparc64")))]
        2_500_000 => libc::B2500000,
        #[cfg(not(any(target_arch = "sparc", target_arch = "sparc64")))]
        3_000_000 => libc::B3000000,
        #[cfg(not(any(target_arch = "sparc", target_arch = "sparc64")))]
        3_500_000 => libc::B3500000,
        #[cfg(not(any(target_arch = "sparc", target_arch = "sparc64")))]
        4_000_000 => libc::B4000000,
        _ => return None,
    })
}

fn syscall_result(result: libc::c_int) -> std::io::Result<()> {
    if result == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, OsStr};
    use std::future::Future;
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::task::Waker;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const TEST_TIMEOUT: Duration = Duration::from_secs(1);

    struct Pty {
        master: std::fs::File,
        slave_path: PathBuf,
    }

    fn open_pty() -> std::io::Result<Pty> {
        let mut master = -1;
        let mut slave = -1;
        syscall_result(unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        })?;
        let master = unsafe { std::fs::File::from_raw_fd(master) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave) };
        let mut path = [0 as libc::c_char; libc::PATH_MAX as usize];
        syscall_result(unsafe {
            libc::ptsname_r(master.as_raw_fd(), path.as_mut_ptr(), path.len())
        })?;
        let path = unsafe { CStr::from_ptr(path.as_ptr()) };
        let slave_path = PathBuf::from(OsStr::from_bytes(path.to_bytes()));
        drop(slave);
        Ok(Pty { master, slave_path })
    }

    fn settings() -> Settings {
        Settings::new(115200)
    }

    async fn open_test_port() -> std::io::Result<(std::fs::File, Port)> {
        let Pty { master, slave_path } = open_pty()?;
        let port = Port::open(slave_path, settings()).await?;
        Ok((master, port))
    }

    async fn timeout_io<T>(future: impl Future<Output = std::io::Result<T>>) -> std::io::Result<T> {
        tokio::time::timeout(TEST_TIMEOUT, future)
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "PTY operation timed out")
            })?
    }

    fn background_read_exact(
        mut master: std::fs::File,
        length: usize,
    ) -> tokio::task::JoinHandle<std::io::Result<Vec<u8>>> {
        tokio::task::spawn_blocking(move || {
            let mut received = vec![0; length];
            master.read_exact(&mut received)?;
            Ok(received)
        })
    }

    async fn background_timeout_io<T>(
        task: tokio::task::JoinHandle<std::io::Result<T>>,
    ) -> std::io::Result<T> {
        timeout_io(async move { task.await.map_err(std::io::Error::other)? }).await
    }

    async fn complete_after_pending<T>(
        future: impl Future<Output = std::io::Result<T>>,
        on_pending: impl FnOnce() -> std::io::Result<()>,
    ) -> std::io::Result<T> {
        let mut future = std::pin::pin!(future);
        let mut on_pending = Some(on_pending);
        let result = timeout_io(std::future::poll_fn(|cx| {
            let result = future.as_mut().poll(cx);
            if result.is_pending()
                && let Some(on_pending) = on_pending.take()
                && let Err(error) = on_pending()
            {
                return Poll::Ready(Err(error));
            }
            result
        }))
        .await;
        assert!(
            on_pending.is_none(),
            "operation completed without first returning Pending"
        );
        result
    }

    #[tokio::test]
    async fn zero_capacity_read_completes_immediately() -> std::io::Result<()> {
        let (_master, mut port) = open_test_port().await?;
        let mut bytes = [];
        let mut read_buf = ReadBuf::new(&mut bytes);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let result = Pin::new(&mut port).poll_read(&mut context, &mut read_buf);
        assert!(matches!(result, Poll::Ready(Ok(()))));
        assert!(read_buf.filled().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn reads_multiple_arrivals() -> std::io::Result<()> {
        let (mut master, mut port) = open_test_port().await?;
        master.write_all(b"hello")?;
        let mut first = [0; 5];
        timeout_io(port.read_exact(&mut first)).await?;
        assert_eq!(&first, b"hello");
        let mut second = [0; 5];
        complete_after_pending(port.read_exact(&mut second), || master.write_all(b"world")).await?;
        assert_eq!(&second, b"world");
        Ok(())
    }

    #[tokio::test]
    async fn pending_read_completes_when_master_closes() -> std::io::Result<()> {
        let (master, mut port) = open_test_port().await?;
        let mut master = Some(master);
        let mut received = [0; 1];
        let result = complete_after_pending(port.read(&mut received), || {
            drop(master.take());
            Ok(())
        })
        .await;
        match result {
            Ok(0) => {}
            Ok(count) => panic!("read returned {count} bytes after PTY hangup"),
            Err(error) if error.raw_os_error() == Some(libc::EIO) => {}
            Err(error) => return Err(error),
        }
        Ok(())
    }

    #[tokio::test]
    async fn writes_to_master() -> std::io::Result<()> {
        let (master, mut port) = open_test_port().await?;
        let expected = b"\x00hello\r\n\x7f\x80\xff";
        let reader = background_read_exact(master, expected.len());
        timeout_io(port.write_all(expected)).await?;
        let received = background_timeout_io(reader).await?;
        assert_eq!(&received, expected);
        Ok(())
    }

    #[tokio::test]
    async fn pending_write_completes_when_master_drains() -> std::io::Result<()> {
        const PAYLOAD_LENGTH: usize = 256 * 1024;
        let (master, mut port) = open_test_port().await?;
        let expected: Vec<_> = (0..PAYLOAD_LENGTH).map(|index| index as u8).collect();
        let mut reader = None;
        complete_after_pending(port.write_all(&expected), || {
            reader = Some(background_read_exact(master, expected.len()));
            Ok(())
        })
        .await?;
        let reader = reader.expect("write returned Pending without starting the PTY reader");
        let received = background_timeout_io(reader).await?;
        assert_eq!(received, expected);
        Ok(())
    }

    fn port_termios(port: impl AsFd) -> std::io::Result<libc::termios> {
        let raw_fd = port.as_fd().as_raw_fd();
        let mut termios = MaybeUninit::<libc::termios>::uninit();
        syscall_result(unsafe { libc::tcgetattr(raw_fd, termios.as_mut_ptr()) })?;
        Ok(unsafe { termios.assume_init() })
    }

    #[tokio::test]
    async fn open_applies_default_settings() -> std::io::Result<()> {
        let Pty {
            master: _master,
            slave_path,
        } = open_pty()?;
        let port = crate::SerialPort::open(slave_path, 9_600).await?;
        let termios = port_termios(&port)?;
        assert_eq!(termios.c_cflag & libc::CSIZE, libc::CS8);
        assert_eq!(termios.c_cflag & libc::PARENB, 0);
        assert_eq!(termios.c_cflag & libc::CSTOPB, 0);
        assert_eq!(termios.c_cflag & libc::CRTSCTS, 0);
        assert_eq!(unsafe { libc::cfgetospeed(&termios) }, libc::B9600);
        Ok(())
    }

    #[test]
    fn applies_configured_settings() -> std::io::Result<()> {
        let Pty {
            master,
            slave_path: _slave_path,
        } = open_pty()?;
        let mut termios = port_termios(master)?;
        let settings = Settings {
            baud_rate: 19_200,
            data_bits: DataBits::Seven,
            parity: Parity::Even,
            stop_bits: StopBits::Two,
            flow_control: FlowControl::Hardware,
        };
        apply_settings(&mut termios, &settings)?;
        assert_eq!(termios.c_cflag & libc::CSIZE, libc::CS7);
        assert_eq!(termios.c_cflag & libc::PARENB, libc::PARENB);
        assert_eq!(termios.c_cflag & libc::PARODD, 0);
        assert_eq!(termios.c_cflag & libc::CSTOPB, libc::CSTOPB);
        assert_eq!(termios.c_cflag & libc::CRTSCTS, libc::CRTSCTS);
        assert_eq!(unsafe { libc::cfgetospeed(&termios) }, libc::B19200);
        Ok(())
    }

    #[tokio::test]
    async fn open_sets_exclusive_mode() -> std::io::Result<()> {
        let Pty {
            master: _master,
            slave_path,
        } = open_pty()?;
        let _port = Port::open(slave_path.clone(), settings()).await?;
        let error = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(slave_path)
            .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EBUSY));
        Ok(())
    }

    #[tokio::test]
    async fn rejects_non_tty_path() {
        const NON_TTY_PATH: &str = "/dev/null";
        let error = match Port::open(PathBuf::from(NON_TTY_PATH), settings()).await {
            Ok(_) => panic!("incorrectly opened {NON_TTY_PATH} as a serial port"),
            Err(error) => error,
        };
        assert!(
            matches!(
                error.raw_os_error(),
                Some(libc::ENODEV) | Some(libc::ENOTTY) | Some(libc::EPERM)
            ),
            "unexpected error while opening non-tty path: {error}"
        );
    }

    #[test]
    fn supports_general_linux_baud_rates() {
        let expected = [
            (50, libc::B50),
            (75, libc::B75),
            (110, libc::B110),
            (134, libc::B134),
            (150, libc::B150),
            (200, libc::B200),
            (300, libc::B300),
            (600, libc::B600),
            (1_200, libc::B1200),
            (1_800, libc::B1800),
            (2_400, libc::B2400),
            (4_800, libc::B4800),
            (9_600, libc::B9600),
            (19_200, libc::B19200),
            (38_400, libc::B38400),
            (57_600, libc::B57600),
            (115_200, libc::B115200),
            (230_400, libc::B230400),
            (460_800, libc::B460800),
            (500_000, libc::B500000),
            (576_000, libc::B576000),
            (921_600, libc::B921600),
            (1_000_000, libc::B1000000),
            (1_152_000, libc::B1152000),
            (1_500_000, libc::B1500000),
            (2_000_000, libc::B2000000),
        ];
        for (baud_rate, speed) in expected {
            assert_eq!(standard_baud_rate(baud_rate), Some(speed));
        }
    }

    #[cfg(any(target_arch = "sparc", target_arch = "sparc64"))]
    #[test]
    fn supports_sparc_baud_rates() {
        assert_eq!(standard_baud_rate(76_800), Some(libc::B76800));
        assert_eq!(standard_baud_rate(153_600), Some(libc::B153600));
        assert_eq!(standard_baud_rate(307_200), Some(libc::B307200));
        assert_eq!(standard_baud_rate(614_400), Some(libc::B614400));
        assert_eq!(standard_baud_rate(4_000_000), None);
    }

    #[cfg(not(any(target_arch = "sparc", target_arch = "sparc64")))]
    #[test]
    fn supports_non_sparc_baud_rates() {
        assert_eq!(standard_baud_rate(2_500_000), Some(libc::B2500000));
        assert_eq!(standard_baud_rate(3_000_000), Some(libc::B3000000));
        assert_eq!(standard_baud_rate(3_500_000), Some(libc::B3500000));
        assert_eq!(standard_baud_rate(4_000_000), Some(libc::B4000000));
        assert_eq!(standard_baud_rate(76_800), None);
    }
}
