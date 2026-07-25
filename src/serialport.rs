use crate::settings::Settings;
use crate::{SerialPortBuilder, sys};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::ReadBuf;

pub struct SerialPort {
    port: sys::Port,
}

impl SerialPort {
    // TODO: Support open options like exclusive mode, etc.
    pub async fn open(path: impl AsRef<Path>, baud_rate: u32) -> std::io::Result<Self> {
        Self::builder(path, baud_rate).open().await
    }

    pub fn builder(path: impl AsRef<Path>, baud_rate: u32) -> SerialPortBuilder {
        SerialPortBuilder::new(path, baud_rate)
    }

    pub(crate) async fn open_with_settings(
        path: PathBuf,
        settings: Settings,
    ) -> std::io::Result<Self> {
        let port = sys::Port::open(path, settings).await?;
        Ok(Self { port })
    }

    // Test-specific accessor for the underlying `sys::Port`
    //
    // TODO: Revisit
    // I don't especially love having any test-specific code hanging out in our impl, but this
    // allows accessing the underlying port without having to specify it anywhere. I could make the
    // `port` field `pub(crate)` or even provide `impl AsFd` long-term, but for now this allows the
    // test construction to work without changing anything outisde of test mode.
    #[cfg(test)]
    pub(crate) fn port(&self) -> &sys::Port {
        &self.port
    }
}

impl tokio::io::AsyncRead for SerialPort {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.port).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for SerialPort {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        Pin::new(&mut this.port).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.port).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.port).poll_shutdown(cx)
    }
}
