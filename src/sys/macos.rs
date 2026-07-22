use crate::Settings;
use std::os::fd::OwnedFd;
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

    // TODO: Configure according to settings

    Ok(fd)
}
