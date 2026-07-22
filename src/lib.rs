//! Cross-platform serialport library for Tokio

pub mod settings;

pub use settings::{DataBits, FlowControl, Parity, Settings, StopBits};
