use crate::{DataBits, FlowControl, Parity, Settings, StopBits};
use std::path::Path;

pub struct SerialPort {}

impl SerialPort {
    // TODO: Support full user-provided settings
    // TODO: Support open options like exclusive mode, etc.
    pub async fn open(_path: impl AsRef<Path>, baud_rate: u32) -> std::io::Result<Self> {
        let _settings = Settings {
            baud_rate,
            data_bits: DataBits::Five,
            parity: Parity::None,
            stop_bits: StopBits::One,
            flow_control: FlowControl::None,
        };
        todo!()
    }
}
