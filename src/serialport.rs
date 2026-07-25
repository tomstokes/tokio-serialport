use crate::settings::Settings;
use crate::{SerialPortBuilder, sys};
#[cfg(unix)]
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::ReadBuf;

pub struct SerialPort {
    port: sys::Port,
}

impl SerialPort {
    // TODO: Support open options like exclusive mode, etc.
    pub async fn open(path: impl Into<PathBuf>, baud_rate: u32) -> std::io::Result<Self> {
        Self::builder(path, baud_rate).open().await
    }

    pub fn builder(path: impl Into<PathBuf>, baud_rate: u32) -> SerialPortBuilder {
        SerialPortBuilder::new(path, baud_rate)
    }

    pub(crate) async fn open_with_settings(
        path: PathBuf,
        settings: Settings,
    ) -> std::io::Result<Self> {
        let port = sys::Port::open(path, settings).await?;
        Ok(Self { port })
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

// TODO: Thorough documentation for AsFd and AsRawFd
// These impls make the library more flexible but open the door to a lot of footguns. The goal of
// this library should be to provide as many of the most common operations through an ergonomic
// API but exposing AsFd and AsRawFd provides an escape hatch for more complex scenarios.
//
// This is also Unix/BSD only. We can introduce AsHandle/AsRawHandle for Windows.
//
// Document some requirements and warnings, like that the fd must remain O_NONBLOCK and any
// settings changed might interfere with functions of the library.

#[cfg(unix)]
impl AsFd for SerialPort {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.port.as_fd()
    }
}

#[cfg(unix)]
impl AsRawFd for SerialPort {
    fn as_raw_fd(&self) -> RawFd {
        self.port.as_raw_fd()
    }
}
