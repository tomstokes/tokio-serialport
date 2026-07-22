use crate::Settings;
use std::os::fd::OwnedFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use tokio::io::unix::AsyncFd;

pub(crate) struct Port {
    fd: AsyncFd<OwnedFd>,
}

impl Port {
    pub(crate) async fn open(path: impl AsRef<Path>, _settings: Settings) -> std::io::Result<Self> {
        // This function takes 2.7ms on my fast Mac with good drivers. The opening really should
        // be done async because this could take a lot longer on slower machines
        // TODO: Make opening async
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY | libc::O_CLOEXEC)
            .open(path)?;
        let fd = OwnedFd::from(file);

        // TODO: Set exclusivity

        // TODO: Configure according to settings

        let async_fd = AsyncFd::new(fd)?;

        Ok(Self { fd: async_fd })
    }
}
