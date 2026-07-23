use crate::{DataBits, FlowControl, Parity, Settings, StopBits};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use tokio::io::unix::AsyncFd;

pub(crate) struct Port {
    fd: AsyncFd<OwnedFd>,
}

impl Port {
    pub(crate) async fn open(path: impl AsRef<Path>, settings: Settings) -> std::io::Result<Self> {
        let path = path.as_ref().to_owned();
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
}

/// Synchronously open and configure a serial port at the given path
///
/// Opening and configuring a serial port takes longer than 1ms in testing and therefore should not
/// be allowed ot block the tokio executor. This function is intended to be used with
/// `tokio::task::spawn_blocking` to run on a separate thread.
fn open_port(path: &Path, settings: Settings) -> std::io::Result<OwnedFd> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY | libc::O_CLOEXEC)
        .open(path)?;
    let fd = OwnedFd::from(file);

    // TODO: Set exclusivity

    configure_port(fd.as_fd(), &settings)?;

    Ok(fd)
}

fn configure_port(fd: BorrowedFd, settings: &Settings) -> std::io::Result<()> {
    // https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/termios.h

    let raw_fd = fd.as_raw_fd();

    // Initialize the `termios` struct with `tcgetattr` using `MaybeUninit`
    let mut termios = MaybeUninit::<libc::termios>::uninit();
    syscall_result(unsafe { libc::tcgetattr(raw_fd, termios.as_mut_ptr()) })?;
    let mut termios = unsafe { termios.assume_init() };

    // Configure the tty to operate in raw mode
    unsafe {
        libc::cfmakeraw(&mut termios);
    }

    // CIGNORE ignores control flags, so ensure it is not set
    termios.c_cflag &= !libc::CIGNORE;

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
            termios.c_cflag |= libc::PARENB;
            // Use odd parity
            termios.c_cflag |= libc::PARODD;
            // Enable input parity checking
            termios.c_iflag |= libc::INPCK;
        }
        Parity::Even => {
            // Enable parity generation on output and parity checking on input
            termios.c_cflag |= libc::PARENB;
            // Enable input parity checking
            termios.c_iflag |= libc::INPCK;
        }
    };

    // Set number of stop bits (`CSTOPB` enabled for 2 stop bits, disabled for 1)
    match settings.stop_bits {
        StopBits::One => termios.c_cflag &= !libc::CSTOPB,
        StopBits::Two => termios.c_cflag |= libc::CSTOPB,
    }

    // Clear flow control flags
    termios.c_iflag &= !(libc::IXON | libc::IXOFF | libc::IXANY);

    // The following two flags are defined in Darwin's termios.h but in libc
    // https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/termios.h
    const CDTR_IFLOW: libc::tcflag_t = 0x0004_0000;
    const CDSR_OFLOW: libc::tcflag_t = 0x0008_0000;

    termios.c_cflag &= !(libc::CRTSCTS | CDTR_IFLOW | CDSR_OFLOW | libc::MDMBUF);

    // Configure flow control
    match settings.flow_control {
        FlowControl::None => {}
        FlowControl::Software => {
            // Enable XON/XOFF flow control on output
            termios.c_iflag |= libc::IXON;
            // Enable XON/XOFF flow control on input
            termios.c_iflag |= libc::IXOFF;

            // Conventional XON/XOFF START character
            termios.c_cc[libc::VSTART] = 0x11;

            // Conventional XON/XOFF STOP character
            termios.c_cc[libc::VSTOP] = 0x13;
        }
        FlowControl::Hardware => {
            // CTS output flow control and RTS input flow control (mac)
            termios.c_cflag |= libc::CRTSCTS;
        }
    }

    // Configure baud rate
    // TODO: Handle non-standard baud rates
    let baud_rate = match settings.baud_rate {
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
        7_200 => libc::B7200,
        9_600 => libc::B9600,
        14_400 => libc::B14400,
        19_200 => libc::B19200,
        28_800 => libc::B28800,
        38_400 => libc::B38400,
        57_600 => libc::B57600,
        76_800 => libc::B76800,
        115_200 => libc::B115200,
        230_400 => libc::B230400,
        other => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unsupported baud rate: {other}"),
            ));
        }
    };
    syscall_result(unsafe { libc::cfsetspeed(&mut termios, baud_rate) })?;

    // Set the termios parameters on the fd
    syscall_result(unsafe { libc::tcsetattr(raw_fd, libc::TCSANOW, &termios) })?;

    Ok(())
}

/// Small helper to convert a syscall result into Result<(), std::io::Error>
fn syscall_result(result: libc::c_int) -> std::io::Result<()> {
    if result == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
