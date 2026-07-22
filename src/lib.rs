//! Cross-platform serialport library for Tokio

pub mod serialport;
pub use serialport::SerialPort;
pub mod settings;

pub use settings::{DataBits, FlowControl, Parity, Settings, StopBits};
