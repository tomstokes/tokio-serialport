//! Cross-platform serialport library for Tokio

extern crate core;

mod builder;
mod serialport;
mod settings;
mod sys;

pub use builder::SerialPortBuilder;
pub use serialport::SerialPort;
pub use settings::{DataBits, FlowControl, Parity, StopBits};
