use crate::{DataBits, FlowControl, Parity, Settings, StopBits, sys};
use std::path::Path;

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
