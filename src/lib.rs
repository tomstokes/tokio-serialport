//! Cross-platform serialport library for Tokio

extern crate core;

pub mod serialport;
pub use serialport::SerialPort;
pub mod settings;
mod sys;

pub use settings::{DataBits, FlowControl, Parity, Settings, StopBits};
