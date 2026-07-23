use crate::{DataBits, FlowControl, Parity, Settings, StopBits, sys};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::ReadBuf;

pub struct SerialPort {
    port: sys::Port,
}

impl SerialPort {
    // TODO: Support full user-provided settings
    // TODO: Support open options like exclusive mode, etc.
    pub async fn open(path: impl AsRef<Path>, baud_rate: u32) -> std::io::Result<Self> {
        let settings = Settings {
            baud_rate,
            data_bits: DataBits::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
            flow_control: FlowControl::None,
        };
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
